use crate::{
    models::{Client, ValidatedInput},
    patch::{
        json::{ClearScope, JsonPatch},
        toml::TomlPatch,
        KeyPath, LivePatch,
    },
    provider_fields as floor,
    storage::{self, Change, FileKind, Paths},
};
use serde::Serialize;
use serde_json::json;
use toml_edit::{value, Item, Table};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub client: Client,
    pub model: String,
    pub selected_models: Vec<String>,
    pub base_url: String,
    pub changed: bool,
    pub config_path: String,
    pub next_steps: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn apply(paths: &Paths, input: ValidatedInput) -> Result<ImportResult, String> {
    let _lock = paths.lock()?;
    let compatibility = crate::compatibility::inspect(paths, input.client);
    if let Some(blocked) = compatibility.blocked {
        return Err(blocked);
    }
    let mut changes = Vec::new();
    match input.client {
        Client::ClaudeDesktop => changes.extend(crate::desktop_config::changes(paths, &input)?),
        Client::Claude | Client::ClaudeVscode => {
            let kind = FileKind::ClaudeSettings;
            let path = paths.target(kind);
            let before = storage::read_optional(&path)?;
            let mut patch = JsonPatch {
                clear: vec![
                    ClearScope {
                        parent: KeyPath::root(),
                        is_floor: floor::claude_floor_top,
                    },
                    ClearScope {
                        parent: KeyPath::new(&["env"]),
                        is_floor: floor::claude_floor_env,
                    },
                ],
                ..JsonPatch::default()
            };
            // Only provider/auth/model settings change; hooks, permissions and MCP remain untouched.
            patch
                .set
                .push((KeyPath::new(&["model"]), json!(input.model)));
            let options: Vec<_> = input
                .selected_models
                .iter()
                .map(|id| json!({"model": id, "label": id, "description": "通过所选中转站调用"}))
                .collect();
            patch.set.push((
                KeyPath::new(&["modelPicker"]),
                json!({"replaceBuiltInOptions": true, "options": options}),
            ));
            patch.set.push((
                KeyPath::new(&["availableModels"]),
                json!(input.selected_models),
            ));
            for (key, val) in [
                ("ANTHROPIC_BASE_URL", input.root.as_str()),
                ("ANTHROPIC_AUTH_TOKEN", input.api_key.as_str()),
                ("ANTHROPIC_DEFAULT_HAIKU_MODEL", input.model.as_str()),
                ("ANTHROPIC_DEFAULT_SONNET_MODEL", input.model.as_str()),
                ("ANTHROPIC_DEFAULT_OPUS_MODEL", input.model.as_str()),
                ("ANTHROPIC_DEFAULT_FABLE_MODEL", input.model.as_str()),
                ("CLAUDE_CODE_SUBAGENT_MODEL", input.model.as_str()),
                ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
            ] {
                patch.set.push((KeyPath::new(&["env", key]), json!(val)));
            }
            let after = String::from_utf8(
                patch
                    .apply(&path, before.as_deref().map(str::as_bytes))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|_| "配置编码错误。")?;
            changes.push(Change {
                kind,
                before,
                after,
            });
            if input.client == Client::ClaudeVscode {
                changes.push(crate::vscode_config::change(paths, &input)?);
            }
        }
        Client::Codex | Client::CodexDesktop => {
            let kind = FileKind::CodexConfig;
            let path = paths.target(kind);
            let before = storage::read_optional(&path)?;
            let parsed = crate::patch::toml::parse(&path, before.as_deref().map(str::as_bytes))
                .map_err(|e| e.to_string())?;
            if parsed
                .get("profile")
                .and_then(Item::as_str)
                .is_some_and(|s| !s.trim().is_empty())
            {
                return Err(
                    "Codex 当前启用了自定义 profile，请先切回默认配置后再导入，避免配置被覆盖。"
                        .into(),
                );
            }
            let mut provider = Table::new();
            provider.insert("name", value("SuperGPT Connect"));
            provider.insert("base_url", value(format!("{}/v1", input.root)));
            provider.insert("wire_api", value("responses"));
            provider.insert("requires_openai_auth", value(true));
            provider.insert("supports_websockets", value(false));
            let mut patch = TomlPatch::default();
            for segments in floor::CODEX_FLOOR_NESTED {
                patch.remove.push(KeyPath::new(segments));
            }
            for key in [
                "model_reasoning_effort",
                "plan_mode_reasoning_effort",
                "review_model",
                "model_catalog_json",
                "openai_base_url",
                "experimental_bearer_token",
                "base_url",
                "wire_api",
            ] {
                patch.remove.push(KeyPath::new(&[key]));
            }
            for (key, val) in [
                ("model", input.model.as_str()),
                ("model_provider", "supergpt-connect"),
                ("cli_auth_credentials_store", "file"),
                ("web_search", "disabled"),
            ] {
                patch.set.push((KeyPath::new(&[key]), value(val)));
            }
            patch.set.push((
                KeyPath::new(&["model_providers", "supergpt-connect"]),
                Item::Table(provider),
            ));
            if let Some(catalog) = &input.catalog {
                let catalog_path = paths.target(FileKind::CodexCatalog);
                patch.set.push((
                    KeyPath::new(&["model_catalog_json"]),
                    value(catalog_path.to_string_lossy().as_ref()),
                ));
                changes.push(Change {
                    kind: FileKind::CodexCatalog,
                    before: storage::read_optional(&catalog_path)?,
                    after: catalog.clone(),
                });
            }
            let after = String::from_utf8(
                patch
                    .apply(&path, before.as_deref().map(str::as_bytes))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|_| "配置编码错误。")?;
            // Match CC Switch's API-key credential mode; saved login is included in the restore snapshot.
            let auth_path = paths.target(FileKind::CodexAuth);
            let auth_before = storage::read_optional(&auth_path)?;
            if let Some(text) = &auth_before {
                serde_json::from_str::<serde_json::Value>(text)
                    .map_err(|_| "原 auth.json 不是有效 JSON，导入已停止。")?;
            }
            let auth = serde_json::to_string_pretty(&json!({"OPENAI_API_KEY":input.api_key}))
                .map_err(|_| "无法生成凭据文件。")?;
            changes.push(Change {
                kind: FileKind::CodexAuth,
                before: auth_before,
                after: auth,
            });
            changes.push(Change {
                kind,
                before,
                after,
            });
        }
    }
    let changed = storage::commit(paths, input.client, changes)?;
    Ok(ImportResult {
        client: input.client,
        model: input.model,
        selected_models: input.selected_models,
        base_url: input.root,
        changed,
        config_path: paths
            .target(FileKind::primary(input.client))
            .to_string_lossy()
            .into_owned(),
        next_steps: crate::compatibility::next_steps(input.client),
        warnings: compatibility.warnings,
    })
}
