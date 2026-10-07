use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use supergpt_connect_lib::{
    adapter,
    models::{Client, ValidatedInput},
    restore::{self, RestorePoint},
    storage::{self, FileKind, Paths},
};

fn input(client: Client, station: &str) -> ValidatedInput {
    let model = if client.is_codex() {
        "gpt-test"
    } else {
        "claude-sonnet-test"
    };
    ValidatedInput {
        client,
        api_key: format!("sk-fixture-only-{station}"),
        root: format!("https://{station}.example.test"),
        model: model.into(),
        selected_models: vec![model.into()],
        catalog: None,
    }
}

fn point(paths: &Paths, client: Client) -> RestorePoint {
    restore::list(paths)
        .into_iter()
        .find(|p| p.client == client.backup_client())
        .unwrap()
}

fn restore(paths: &Paths, client: Client, overwrite: bool) -> restore::RestoreResult {
    let preview = point(paths, client);
    assert!(preview.available);
    restore::apply(
        paths,
        client,
        preview.review_id.as_deref().unwrap(),
        overwrite,
    )
    .unwrap()
}

fn read(paths: &Paths, kind: FileKind) -> String {
    fs::read_to_string(paths.target(kind)).unwrap()
}

fn write(paths: &Paths, kind: FileKind, text: &str) {
    storage::atomic_write(&paths.target(kind), text.as_bytes()).unwrap();
}

#[test]
fn changing_stations_keeps_initial_config_and_login_and_adds_new_files_to_baseline() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let config = "# original user comment\nmodel = \"original\"\n";
    let auth = "{\n  \"tokens\": {\"refresh_token\": \"fake-original-oauth\"}\n}\n";
    write(&paths, FileKind::CodexConfig, config);
    write(&paths, FileKind::CodexAuth, auth);
    adapter::apply(&paths, input(Client::Codex, "one")).unwrap();
    let first_timestamp = point(&paths, Client::Codex).created_at;
    let mut next = input(Client::CodexDesktop, "two");
    next.catalog = Some("{\"models\":[]}".into());
    adapter::apply(&paths, next).unwrap();
    assert_eq!(point(&paths, Client::Codex).created_at, first_timestamp);
    assert_eq!(point(&paths, Client::Codex).files.len(), 3);
    let latest_config = read(&paths, FileKind::CodexConfig);
    let latest_auth = read(&paths, FileKind::CodexAuth);
    let result = restore(&paths, Client::CodexDesktop, false);
    assert_eq!(result.client, Client::Codex);
    assert_eq!(result.restored_files, 3);
    assert_eq!(read(&paths, FileKind::CodexConfig), config);
    assert_eq!(read(&paths, FileKind::CodexAuth), auth);
    assert!(!paths.target(FileKind::CodexCatalog).exists());
    assert!(!paths.has_backup(Client::Codex));
    let snapshot: Value =
        serde_json::from_slice(&fs::read(&result.safety_backup_path).unwrap()).unwrap();
    let files = snapshot["files"].as_array().unwrap();
    assert_eq!(
        files.iter().find(|f| f["kind"] == "CodexConfig").unwrap()["contents"],
        latest_config
    );
    assert_eq!(
        files.iter().find(|f| f["kind"] == "CodexAuth").unwrap()["contents"],
        latest_auth
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&result.safety_backup_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(paths.state.join("recovery"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}

#[test]
fn desktop_restores_original_mode_and_gateway_after_multiple_imports() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original = "{\"deploymentMode\":\"1p\",\"preferences\":{\"theme\":\"dark\"}}\n";
    let meta = "{\"entries\":[{\"id\":\"work\",\"name\":\"Work\"}],\"appliedId\":\"work\"}";
    write(&paths, FileKind::ClaudeDesktopConfig, original);
    write(&paths, FileKind::ClaudeDesktopMeta, meta);
    adapter::apply(&paths, input(Client::ClaudeDesktop, "one")).unwrap();
    adapter::apply(&paths, input(Client::ClaudeDesktop, "two")).unwrap();
    let result = restore(&paths, Client::ClaudeDesktop, false);
    assert_eq!(result.restored_files, 4);
    assert_eq!(read(&paths, FileKind::ClaudeDesktopConfig), original);
    assert_eq!(read(&paths, FileKind::ClaudeDesktopMeta), meta);
    assert!(!paths.target(FileKind::ClaudeDesktopProfile).exists());
    assert!(!paths.target(FileKind::ClaudeThreepConfig).exists());
}

#[test]
fn later_edits_need_acknowledgement_and_are_saved_before_restore() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original = "{\"model\":\"old\",\"env\":{\"ANTHROPIC_API_KEY\":\"fake-old\"}}\n";
    write(&paths, FileKind::ClaudeSettings, original);
    adapter::apply(&paths, input(Client::Claude, "one")).unwrap();
    let later = format!("{}\n", read(&paths, FileKind::ClaudeSettings));
    write(&paths, FileKind::ClaudeSettings, &later);
    let preview = point(&paths, Client::Claude);
    assert!(preview.files[0].modified_since_import);
    let error = restore::apply(
        &paths,
        Client::Claude,
        preview.review_id.as_deref().unwrap(),
        false,
    )
    .err()
    .unwrap();
    assert!(error.contains("新改动"));
    assert_eq!(read(&paths, FileKind::ClaudeSettings), later);
    assert!(!paths.state.join("recovery").exists());
    let result = restore(&paths, Client::Claude, true);
    let snapshot: Value =
        serde_json::from_slice(&fs::read(result.safety_backup_path).unwrap()).unwrap();
    assert_eq!(snapshot["files"][0]["contents"], later);
    assert_eq!(read(&paths, FileKind::ClaudeSettings), original);
}

#[test]
fn stale_preview_cannot_overwrite_a_newer_file_even_with_acknowledgement() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    adapter::apply(&paths, input(Client::Claude, "one")).unwrap();
    let preview = point(&paths, Client::Claude);
    let newer = "{\"model\":\"later-user-choice\"}";
    write(&paths, FileKind::ClaudeSettings, newer);
    let error = restore::apply(
        &paths,
        Client::Claude,
        preview.review_id.as_deref().unwrap(),
        true,
    )
    .err()
    .unwrap();
    assert!(error.contains("已经变化"));
    assert_eq!(read(&paths, FileKind::ClaudeSettings), newer);
    assert!(paths.has_backup(Client::Claude));
    assert!(!paths.state.join("recovery").exists());
}

#[test]
fn stale_preview_cannot_restore_a_different_backup() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    adapter::apply(&paths, input(Client::Claude, "one")).unwrap();
    let preview = point(&paths, Client::Claude);
    let backup = paths.state.join("claude.backup.json");
    let mut record: Value = serde_json::from_slice(&fs::read(&backup).unwrap()).unwrap();
    record["entries"][0]["before"] = json!("{\"model\":\"different-baseline\"}");
    storage::atomic_write(&backup, &serde_json::to_vec(&record).unwrap()).unwrap();
    let current = read(&paths, FileKind::ClaudeSettings);
    assert!(restore::apply(
        &paths,
        Client::Claude,
        preview.review_id.as_deref().unwrap(),
        true
    )
    .is_err());
    assert_eq!(read(&paths, FileKind::ClaudeSettings), current);
}

#[test]
fn legacy_backups_without_timestamp_restore_and_preview_never_contains_keys() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original = "{\"env\":{\"ANTHROPIC_API_KEY\":\"fake-private-before\"}}";
    write(&paths, FileKind::ClaudeSettings, original);
    adapter::apply(&paths, input(Client::Claude, "one")).unwrap();
    let backup = paths.state.join("claude.backup.json");
    let mut record: Value = serde_json::from_slice(&fs::read(&backup).unwrap()).unwrap();
    record.as_object_mut().unwrap().remove("createdAt");
    storage::atomic_write(&backup, &serde_json::to_vec(&record).unwrap()).unwrap();
    let previews = serde_json::to_string(&restore::list(&paths)).unwrap();
    assert!(!previews.contains("fake-private-before"));
    assert!(!previews.contains("sk-fixture-only"));
    assert!(!previews.contains("ANTHROPIC_API_KEY"));
    assert!(point(&paths, Client::Claude).created_at.is_some());
    restore(&paths, Client::Claude, false);
    assert_eq!(read(&paths, FileKind::ClaudeSettings), original);
}

#[test]
fn pending_operation_is_undone_before_the_original_restore_point() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original = "{\"model\":\"original\"}\n";
    write(&paths, FileKind::ClaudeSettings, original);
    adapter::apply(&paths, input(Client::Claude, "one")).unwrap();
    let last_working = read(&paths, FileKind::ClaudeSettings);
    let interrupted = "{\"model\":\"partial-import\"}";
    let pending = json!({"version":1,"client":"claude","entries":[{
        "kind":"ClaudeSettings", "path":paths.target(FileKind::ClaudeSettings),
        "before":last_working, "after_hash":format!("{:x}", Sha256::digest(interrupted.as_bytes()))
    }]});
    storage::atomic_write(
        &paths.state.join("claude.pending.json"),
        &serde_json::to_vec(&pending).unwrap(),
    )
    .unwrap();
    write(&paths, FileKind::ClaudeSettings, interrupted);
    assert!(point(&paths, Client::Claude).pending);
    assert!(restore(&paths, Client::Claude, false).was_pending);
    assert_eq!(read(&paths, FileKind::ClaudeSettings), last_working);
    assert!(!point(&paths, Client::Claude).pending);
    assert!(point(&paths, Client::Claude).available);
    restore(&paths, Client::Claude, false);
    assert_eq!(read(&paths, FileKind::ClaudeSettings), original);
}

#[test]
fn missing_or_damaged_backup_does_not_reset_existing_configuration() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let original = "{\"model\":\"keep-me\"}";
    write(&paths, FileKind::ClaudeSettings, original);
    assert!(!point(&paths, Client::Claude).available);
    assert!(restore::apply(&paths, Client::Claude, "invalid", true).is_err());
    assert_eq!(read(&paths, FileKind::ClaudeSettings), original);
    adapter::apply(&paths, input(Client::Claude, "one")).unwrap();
    let current = read(&paths, FileKind::ClaudeSettings);
    let backup = paths.state.join("claude.backup.json");
    storage::atomic_write(&backup, b"{broken-backup").unwrap();
    assert!(point(&paths, Client::Claude).error.is_some());
    assert!(!point(&paths, Client::Claude).available);
    assert!(restore::apply(&paths, Client::Claude, "invalid", true).is_err());
    assert!(adapter::apply(&paths, input(Client::Claude, "two")).is_err());
    assert_eq!(read(&paths, FileKind::ClaudeSettings), current);
    assert_eq!(fs::read_to_string(backup).unwrap(), "{broken-backup");
}

#[test]
fn vscode_restore_returns_both_shared_settings_and_jsonc_exactly() {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::for_home(home.path());
    let settings = "{\"model\":\"original\"}\n";
    let vscode = "{\n  // original comment\n  \"editor.fontSize\": 17,\n}\n";
    write(&paths, FileKind::ClaudeSettings, settings);
    write(&paths, FileKind::ClaudeVscode, vscode);
    adapter::apply(&paths, input(Client::ClaudeVscode, "one")).unwrap();
    assert_eq!(
        restore(&paths, Client::ClaudeVscode, false).restored_files,
        2
    );
    assert_eq!(read(&paths, FileKind::ClaudeSettings), settings);
    assert_eq!(read(&paths, FileKind::ClaudeVscode), vscode);
}
