use serde_json::{json, Value};
use std::fs;
use supergpt_connect_lib::{
    adapter,
    desktop_config::PROFILE_ID,
    models::{Client, ValidatedInput},
    storage::{self, FileKind, Paths},
};

fn input(client: Client) -> ValidatedInput {
    ValidatedInput {
        client,
        api_key: "sk-isolated-test-only".into(),
        root: "https://gateway.example.test".into(),
        model: "claude-opus-test".into(),
        selected_models: vec!["claude-sonnet-test".into(), "claude-opus-test".into()],
        catalog: None,
    }
}
fn read(paths: &Paths, kind: FileKind) -> Value {
    serde_json::from_str(&fs::read_to_string(paths.target(kind)).unwrap()).unwrap()
}
fn write(paths: &Paths, kind: FileKind, text: &str) {
    storage::atomic_write(&paths.target(kind), text.as_bytes()).unwrap();
}

#[test]
fn desktop_activates_gateway_all_four_files_and_restores_existing_profiles() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let config = "{\"preferences\":{\"theme\":\"dark\"},\"mcpServers\":{\"keep\":{}},\"deploymentMode\":\"1p\"}";
    let meta = "{\"entries\":[{\"id\":\"work\",\"name\":\"Work\",\"extra\":true}],\"appliedId\":\"work\",\"keep\":1}";
    write(&paths, FileKind::ClaudeDesktopConfig, config);
    write(&paths, FileKind::ClaudeDesktopMeta, meta);
    let result = adapter::apply(&paths, input(Client::ClaudeDesktop)).unwrap();
    assert!(result.changed);
    assert!(result.next_steps.iter().any(|s| s.contains("Cowork")));
    let profile = read(&paths, FileKind::ClaudeDesktopProfile);
    assert_eq!(
        profile["inferenceModels"],
        json!(["claude-opus-test", "claude-sonnet-test"])
    );
    assert_eq!(profile["inferenceProvider"], "gateway");
    assert_eq!(profile["inferenceCredentialKind"], "static");
    assert_eq!(profile["modelDiscoveryEnabled"], false);
    assert!(profile.get("coworkEgressAllowedHosts").is_none());
    assert_eq!(
        read(&paths, FileKind::ClaudeDesktopMeta)["appliedId"],
        PROFILE_ID
    );
    assert_eq!(
        read(&paths, FileKind::ClaudeDesktopMeta)["entries"][0],
        json!({"id":"work","name":"Work","extra":true})
    );
    for kind in [FileKind::ClaudeDesktopConfig, FileKind::ClaudeThreepConfig] {
        assert_eq!(read(&paths, kind)["deploymentMode"], "3p");
    }
    assert_eq!(
        read(&paths, FileKind::ClaudeDesktopConfig)["preferences"]["theme"],
        "dark"
    );
    assert!(!paths.target(FileKind::ClaudeSettings).exists());
    assert!(
        !adapter::apply(&paths, input(Client::ClaudeDesktop))
            .unwrap()
            .changed
    );
    storage::restore(&paths, Client::ClaudeDesktop).unwrap();
    assert_eq!(
        fs::read_to_string(paths.target(FileKind::ClaudeDesktopConfig)).unwrap(),
        config
    );
    assert_eq!(
        fs::read_to_string(paths.target(FileKind::ClaudeDesktopMeta)).unwrap(),
        meta
    );
    assert!(!paths.target(FileKind::ClaudeDesktopProfile).exists());
    assert!(!paths.target(FileKind::ClaudeThreepConfig).exists());
}

#[test]
fn malformed_desktop_registry_or_non_claude_model_never_half_activates() {
    for (meta, model) in [
        ("{\"entries\":42}", "claude-sonnet-test"),
        ("{\"entries\":[{}]}", "claude-sonnet-test"),
        ("{}", "gpt-test"),
    ] {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::for_home(home.path());
        write(&paths, FileKind::ClaudeDesktopMeta, meta);
        let mut data = input(Client::ClaudeDesktop);
        data.model = model.into();
        data.selected_models = vec![model.into()];
        assert!(adapter::apply(&paths, data).is_err());
        assert!(!paths.target(FileKind::ClaudeDesktopConfig).exists());
        assert!(!paths.target(FileKind::ClaudeDesktopProfile).exists());
        assert_eq!(
            fs::read_to_string(paths.target(FileKind::ClaudeDesktopMeta)).unwrap(),
            meta
        );
    }
}

#[test]
fn desktop_restore_preserves_later_user_edits() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    adapter::apply(&paths, input(Client::ClaudeDesktop)).unwrap();
    let mut data = read(&paths, FileKind::ClaudeDesktopConfig);
    data["preferences"] = json!({"theme":"light"});
    write(&paths, FileKind::ClaudeDesktopConfig, &data.to_string());
    assert!(storage::restore(&paths, Client::ClaudeDesktop).is_err());
    assert!(paths.has_backup(Client::ClaudeDesktop));
    assert_eq!(read(&paths, FileKind::ClaudeDesktopConfig), data);
}

#[test]
fn vscode_updates_login_environment_preserves_jsonc_and_restores_both_files() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original="{\n  // 保留用户注释与 URL\n  \"editor.fontSize\": 15,\n  \"custom.url\": \"https://example.test/a//b\",\n  \"claudeCode.environmentVariables\": [\n    {\"name\":\"ANTHROPIC_API_KEY\",\"value\":\"old-fake\"},\n    {\"name\":\"CLAUDE_CODE_USE_BEDROCK\",\"value\":\"1\"},\n    {\"name\":\"KEEP_ME\",\"value\":\"yes\"},\n  ],\n}\n";
    write(&paths, FileKind::ClaudeVscode, original);
    adapter::apply(&paths, input(Client::ClaudeVscode)).unwrap();
    let text = fs::read_to_string(paths.target(FileKind::ClaudeVscode)).unwrap();
    assert!(text.contains("// 保留用户注释与 URL"));
    assert!(text.contains("\"editor.fontSize\": 15,"));
    let doc: Value = jsonc_parser::parse_to_serde_value(&text, &Default::default()).unwrap();
    let vars = doc["claudeCode.environmentVariables"].as_array().unwrap();
    assert!(
        vars.iter()
            .any(|v| v["name"] == "ANTHROPIC_BASE_URL"
                && v["value"] == "https://gateway.example.test")
    );
    assert!(vars
        .iter()
        .any(|v| v["name"] == "KEEP_ME" && v["value"] == "yes"));
    assert!(!vars
        .iter()
        .any(|v| v["name"] == "ANTHROPIC_API_KEY" || v["name"] == "CLAUDE_CODE_USE_BEDROCK"));
    assert_eq!(
        read(&paths, FileKind::ClaudeSettings)["model"],
        "claude-opus-test"
    );
    storage::restore(&paths, Client::ClaudeVscode).unwrap();
    assert_eq!(
        fs::read_to_string(paths.target(FileKind::ClaudeVscode)).unwrap(),
        original
    );
    assert!(!paths.target(FileKind::ClaudeSettings).exists());
}

#[test]
fn broken_vscode_config_leaves_claude_settings_untouched() {
    for text in [
        "{broken",
        "[]",
        "{\"claudeCode.environmentVariables\":false}",
        "{\"claudeCode.environmentVariables\":[],\"claudeCode.environmentVariables\":[]}",
        "{\"a\":1 \"b\":2}",
        "{'a':1}",
        "{unquoted:1}",
    ] {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::for_home(home.path());
        write(&paths, FileKind::ClaudeVscode, text);
        assert!(adapter::apply(&paths, input(Client::ClaudeVscode)).is_err());
        assert!(!paths.target(FileKind::ClaudeSettings).exists());
    }
}

#[test]
fn codex_desktop_and_cli_share_catalog_credentials_and_restore_point() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let mut data = input(Client::CodexDesktop);
    data.model = "gpt-fixture".into();
    data.selected_models = vec![data.model.clone()];
    data.catalog = Some("{\"models\":[]}".into());
    let result = adapter::apply(&paths, data).unwrap();
    assert_eq!(result.client, Client::CodexDesktop);
    assert!(paths.has_backup(Client::CodexDesktop));
    assert!(paths.has_backup(Client::Codex));
    let doc = fs::read_to_string(paths.target(FileKind::CodexConfig))
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    assert_eq!(doc["cli_auth_credentials_store"].as_str(), Some("file"));
    assert_eq!(doc["model_provider"].as_str(), Some("supergpt-connect"));
    assert!(doc["model_catalog_json"]
        .as_str()
        .unwrap()
        .ends_with("supergpt-models.json"));
    storage::restore(&paths, Client::CodexDesktop).unwrap();
    assert!(!paths.has_backup(Client::Codex));
    for kind in [
        FileKind::CodexConfig,
        FileKind::CodexAuth,
        FileKind::CodexCatalog,
    ] {
        assert!(!paths.target(kind).exists());
    }
}
