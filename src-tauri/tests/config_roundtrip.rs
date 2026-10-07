use std::fs;
use supergpt_connect_lib::{
    adapter,
    models::{Client, ValidatedInput},
    storage::{self, FileKind, Paths},
};

fn input(client: Client) -> ValidatedInput {
    ValidatedInput {
        client,
        api_key: "sk-fake-test-only".into(),
        root: "https://api.example.test".into(),
        model: match client {
            Client::Claude | Client::ClaudeDesktop | Client::ClaudeVscode => "claude-sonnet-test",
            Client::Codex | Client::CodexDesktop => "gpt-test-codex",
        }
        .into(),
        selected_models: vec![match client {
            Client::Claude | Client::ClaudeDesktop | Client::ClaudeVscode => "claude-sonnet-test",
            Client::Codex | Client::CodexDesktop => "gpt-test-codex",
        }
        .into()],
        catalog: None,
    }
}

#[test]
fn codex_merge_keeps_comments_mcp_and_permissions_and_restores_login_exactly() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original="# My comment\nmodel = \"old\" # model note\napproval_policy = \"on-request\"\n\n[mcp_servers.example]\ncommand = \"my-server\"\n\n[model_providers.original]\nname = \"Original\"\n";
    let auth = "{\n  \"tokens\": {\"refresh_token\": \"fake-test-oauth\"}\n}\n";
    storage::atomic_write(&paths.target(FileKind::CodexConfig), original.as_bytes()).unwrap();
    storage::atomic_write(&paths.target(FileKind::CodexAuth), auth.as_bytes()).unwrap();
    let result = adapter::apply(&paths, input(Client::Codex)).unwrap();
    assert!(result.changed);
    let merged = fs::read_to_string(paths.target(FileKind::CodexConfig)).unwrap();
    assert!(merged.contains("# My comment"));
    assert!(merged.contains("# model note"));
    assert!(merged.contains("command = \"my-server\""));
    assert!(merged.contains("approval_policy = \"on-request\""));
    assert!(merged.contains("[model_providers.original]"));
    let doc = merged.parse::<toml_edit::DocumentMut>().unwrap();
    assert_eq!(doc["model_provider"].as_str(), Some("supergpt-connect"));
    assert_eq!(
        doc["model_providers"]["supergpt-connect"]["base_url"].as_str(),
        Some("https://api.example.test/v1")
    );
    let encoded = serde_json::to_string(&result).unwrap();
    assert!(!encoded.contains("sk-fake-test-only"));
    storage::restore(&paths, Client::Codex).unwrap();
    assert_eq!(
        fs::read_to_string(paths.target(FileKind::CodexConfig)).unwrap(),
        original
    );
    assert_eq!(
        fs::read_to_string(paths.target(FileKind::CodexAuth)).unwrap(),
        auth
    );
}

#[test]
fn new_install_and_repeated_import_keep_the_original_restore_point() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    adapter::apply(&paths, input(Client::Codex)).unwrap();
    let backup = fs::read(paths.state.join("codex.backup.json")).unwrap();
    assert!(
        !adapter::apply(&paths, input(Client::Codex))
            .unwrap()
            .changed
    );
    assert_eq!(
        backup,
        fs::read(paths.state.join("codex.backup.json")).unwrap()
    );
    storage::restore(&paths, Client::Codex).unwrap();
    assert!(!paths.target(FileKind::CodexAuth).exists());
    assert!(!paths.target(FileKind::CodexConfig).exists());
}

#[test]
fn claude_merge_preserves_hooks_permissions_unrelated_env_and_bom() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original="\u{feff}{\r\n\t\"hooks\": {\"Stop\": []},\r\n\t\"permissions\": {\"allow\": [\"Read\"]},\r\n\t\"env\": {\"CUSTOM_VAR\": \"keep\", \"ANTHROPIC_API_KEY\": \"old-test\", \"CLAUDE_CODE_USE_BEDROCK\": \"1\"}\r\n}\r\n";
    storage::atomic_write(&paths.target(FileKind::ClaudeSettings), original.as_bytes()).unwrap();
    adapter::apply(&paths, input(Client::Claude)).unwrap();
    let merged = fs::read_to_string(paths.target(FileKind::ClaudeSettings)).unwrap();
    assert!(merged.starts_with('\u{feff}'));
    assert!(merged.contains("\r\n\t"));
    let doc: serde_json::Value =
        serde_json::from_str(merged.trim_start_matches('\u{feff}')).unwrap();
    assert_eq!(doc["hooks"]["Stop"], serde_json::json!([]));
    assert_eq!(doc["permissions"]["allow"], serde_json::json!(["Read"]));
    assert_eq!(doc["env"]["CUSTOM_VAR"], "keep");
    assert!(doc["env"].get("ANTHROPIC_API_KEY").is_none());
    assert!(doc["env"].get("CLAUDE_CODE_USE_BEDROCK").is_none());
    storage::restore(&paths, Client::Claude).unwrap();
    assert_eq!(
        fs::read_to_string(paths.target(FileKind::ClaudeSettings)).unwrap(),
        original
    );
}

#[test]
fn malformed_configs_and_active_profiles_make_no_client_changes() {
    for (client, kind, text) in [
        (Client::Claude, FileKind::ClaudeSettings, "{broken"),
        (Client::Claude, FileKind::ClaudeSettings, "{\"env\":42}"),
        (Client::Codex, FileKind::CodexConfig, "model = ["),
        (Client::Codex, FileKind::CodexConfig, "profile = \"work\"\n"),
    ] {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::for_home(home.path());
        storage::atomic_write(&paths.target(kind), text.as_bytes()).unwrap();
        assert!(adapter::apply(&paths, input(client)).is_err());
        assert_eq!(fs::read_to_string(paths.target(kind)).unwrap(), text);
        assert!(!paths.has_backup(client));
        assert!(!paths.target(FileKind::CodexAuth).exists());
    }
}

#[test]
fn restore_refuses_to_destroy_later_user_edits_or_a_different_config_directory() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    adapter::apply(&paths, input(Client::Codex)).unwrap();
    let mut redirected = paths.clone();
    redirected.codex = home.path().join("other-codex");
    assert!(storage::restore(&redirected, Client::Codex)
        .unwrap_err()
        .contains("目录已改变"));
    let config = paths.target(FileKind::CodexConfig);
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str("\n# My later edit\n");
    storage::atomic_write(&config, text.as_bytes()).unwrap();
    let auth = fs::read(paths.target(FileKind::CodexAuth)).unwrap();
    assert!(storage::restore(&paths, Client::Codex)
        .unwrap_err()
        .contains("又被修改"));
    assert_eq!(fs::read_to_string(&config).unwrap(), text);
    assert_eq!(fs::read(paths.target(FileKind::CodexAuth)).unwrap(), auth);
    assert!(paths.has_backup(Client::Codex));
}

#[test]
fn optional_codex_catalog_is_installed_and_restored_with_the_config() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let mut data = input(Client::Codex);
    data.catalog = Some("{\"models\":[{\"slug\":\"gpt-test-codex\"}]}".into());
    adapter::apply(&paths, data).unwrap();
    assert!(paths.target(FileKind::CodexCatalog).is_file());
    let config = fs::read_to_string(paths.target(FileKind::CodexConfig)).unwrap();
    assert!(config.contains("model_catalog_json"));
    storage::restore(&paths, Client::Codex).unwrap();
    assert!(!paths.target(FileKind::CodexCatalog).exists());
}

#[cfg(unix)]
#[test]
fn private_permissions_and_symlink_protection() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    adapter::apply(&paths, input(Client::Claude)).unwrap();
    for path in [
        paths.target(FileKind::ClaudeSettings),
        paths.state.join("claude.backup.json"),
    ] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    storage::restore(&paths, Client::Claude).unwrap();
    let outside = home.path().join("important.json");
    fs::write(&outside, "{}").unwrap();
    symlink(&outside, paths.target(FileKind::ClaudeSettings)).unwrap();
    assert!(adapter::apply(&paths, input(Client::Claude))
        .unwrap_err()
        .contains("符号链接"));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "{}");
}

#[test]
fn claude_picker_contains_exactly_selected_models_and_default_can_be_changed_later() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original = r#"{"model":"old","modelPicker":{"options":[{"model":"old"}]},"availableModels":["old"],"env":{"ANTHROPIC_MODEL":"old"}}"#;
    storage::atomic_write(&paths.target(FileKind::ClaudeSettings), original.as_bytes()).unwrap();
    let mut selected = input(Client::Claude);
    selected.selected_models = vec!["claude-sonnet-test".into(), "custom-team-model".into()];
    selected.model = "custom-team-model".into();
    adapter::apply(&paths, selected).unwrap();
    let result: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(paths.target(FileKind::ClaudeSettings)).unwrap())
            .unwrap();
    assert_eq!(result["model"], "custom-team-model");
    assert_eq!(
        result["availableModels"],
        serde_json::json!(["claude-sonnet-test", "custom-team-model"])
    );
    let picker = &result["modelPicker"];
    assert_eq!(picker["replaceBuiltInOptions"], true);
    assert_eq!(picker["options"].as_array().unwrap().len(), 2);
    assert_eq!(picker["options"][1]["model"], "custom-team-model");
    assert!(result["env"].get("ANTHROPIC_MODEL").is_none());
    storage::restore(&paths, Client::Claude).unwrap();
    assert_eq!(
        fs::read_to_string(paths.target(FileKind::ClaudeSettings)).unwrap(),
        original
    );
}
