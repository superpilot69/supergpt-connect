//! Platform checks use only temporary user directories, including Unicode and spaces.
use std::fs;
use supergpt_connect_lib::{
    adapter,
    models::{Client, ValidatedInput},
    restore,
    storage::{self, FileKind, Paths},
};

#[test]
fn unicode_home_catalog_and_original_crlf_auth_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("中文用户 Test");
    let paths = Paths::for_home(&home);
    let config = "# original\r\nmodel = \"original\"\r\n";
    let auth = "{\r\n  \"tokens\": {\"refresh_token\": \"fake-original\"}\r\n}\r\n";
    storage::atomic_write(&paths.target(FileKind::CodexConfig), config.as_bytes()).unwrap();
    storage::atomic_write(&paths.target(FileKind::CodexAuth), auth.as_bytes()).unwrap();
    adapter::apply(
        &paths,
        ValidatedInput {
            client: Client::CodexDesktop,
            api_key: "sk-isolated-test-only".into(),
            root: "https://gateway.example.test".into(),
            model: "gpt-fixture".into(),
            selected_models: vec!["gpt-fixture".into()],
            catalog: Some("{\"models\":[]}".into()),
        },
    )
    .unwrap();
    let imported = fs::read_to_string(paths.target(FileKind::CodexConfig))
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    assert_eq!(
        std::path::Path::new(imported["model_catalog_json"].as_str().unwrap()),
        paths.target(FileKind::CodexCatalog)
    );
    let point = restore::list(&paths)
        .into_iter()
        .find(|p| p.client == Client::Codex)
        .unwrap();
    restore::apply(
        &paths,
        Client::Codex,
        point.review_id.as_deref().unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(
        fs::read(paths.target(FileKind::CodexConfig)).unwrap(),
        config.as_bytes()
    );
    assert_eq!(
        fs::read(paths.target(FileKind::CodexAuth)).unwrap(),
        auth.as_bytes()
    );
    assert!(!paths.target(FileKind::CodexCatalog).exists());
}

#[cfg(windows)]
#[test]
fn windows_uses_local_claude_roaming_vscode_and_shared_codex() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("用户 Test");
    let paths = Paths::for_home(&home);
    assert_eq!(paths.claude_desktop, home.join("AppData/Local/Claude"));
    assert_eq!(paths.claude_threep, home.join("AppData/Local/Claude-3p"));
    assert_eq!(
        paths.vscode,
        home.join("AppData/Roaming/Code/User/settings.json")
    );
    assert_eq!(paths.claude, home.join(".claude"));
    assert_eq!(paths.codex, home.join(".codex"));
}
