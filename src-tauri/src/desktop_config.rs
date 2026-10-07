// Claude Desktop activation layout adapted from CC Switch (MIT),
// claude_desktop_config.rs. See UPSTREAM.md for provenance.
use crate::{
    models::ValidatedInput,
    patch::{json::JsonPatch, KeyPath, LivePatch},
    storage::{self, Change, FileKind, Paths},
};
use serde_json::{json, Value};

// A separate profile keeps the user's other gateways and CC Switch profiles intact.
pub const PROFILE_ID: &str = "761c488f-5a98-4c98-bac9-8b667c5be40a";

fn object(text: Option<&str>) -> Result<Value, String> {
    let value: Value = serde_json::from_str(text.unwrap_or("{}"))
        .map_err(|_| "Claude 桌面配置不是有效 JSON，未修改配置。")?;
    if !value.is_object() {
        return Err("Claude 桌面配置格式不正确，未修改配置。".into());
    }
    Ok(value)
}

fn patch(paths: &Paths, kind: FileKind, fields: Value) -> Result<Change, String> {
    let path = paths.target(kind);
    let before = storage::read_optional(&path)?;
    let set = fields
        .as_object()
        .ok_or("桌面配置字段无效。")?
        .iter()
        .map(|(key, value)| (KeyPath::new(&[key]), value.clone()))
        .collect();
    let bytes = JsonPatch {
        set,
        ..JsonPatch::default()
    }
    .apply(&path, before.as_deref().map(str::as_bytes))
    .map_err(|e| e.to_string())?;
    Ok(Change {
        kind,
        before,
        after: String::from_utf8(bytes).map_err(|_| "配置编码错误。")?,
    })
}

pub fn changes(paths: &Paths, input: &ValidatedInput) -> Result<Vec<Change>, String> {
    // The desktop menu validates Claude family IDs. Do not silently substitute a
    // different model or pretend arbitrary OpenAI aliases are Claude models.
    if input
        .selected_models
        .iter()
        .any(|id| !supported_model_id(id))
    {
        return Err("Claude 桌面版需要 claude-sonnet-*、claude-opus-*、claude-haiku-* 或 claude-fable-* 模型 ID。请只勾选兼容模型；其他别名需要中转站提供 Claude 路由。".into());
    }
    let mut ordered = vec![input.model.clone()];
    ordered.extend(
        input
            .selected_models
            .iter()
            .filter(|id| **id != input.model)
            .cloned(),
    );
    let profile = patch(
        paths,
        FileKind::ClaudeDesktopProfile,
        json!({
            "inferenceProvider": "gateway",
            "inferenceGatewayBaseUrl": input.root,
            "inferenceGatewayApiKey": input.api_key,
            "inferenceGatewayAuthScheme": "bearer",
            "inferenceCredentialKind": "static",
            "modelDiscoveryEnabled": false,
            "inferenceModels": ordered,
        }),
    )?;
    let before = storage::read_optional(&paths.target(FileKind::ClaudeDesktopMeta))?;
    let mut meta = object(before.as_deref())?;
    let entries = meta
        .as_object_mut()
        .unwrap()
        .entry("entries")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or("Claude 桌面配置目录的 entries 格式无效，未修改配置。")?;
    if entries
        .iter()
        .any(|v| !v.is_object() || v.get("id").and_then(Value::as_str).is_none())
    {
        return Err("Claude 桌面配置目录包含无法识别的条目，未修改配置。".into());
    }
    if let Some(entry) = entries.iter_mut().find(|v| v["id"] == PROFILE_ID) {
        entry["name"] = json!("SuperGPT Connect");
    } else {
        entries.push(json!({ "id": PROFILE_ID, "name": "SuperGPT Connect" }));
    }
    meta["appliedId"] = json!(PROFILE_ID);
    // Activate only after the credential, catalog and registry are ready.
    Ok(vec![
        profile,
        Change {
            kind: FileKind::ClaudeDesktopMeta,
            before,
            after: serde_json::to_string_pretty(&meta).map_err(|_| "无法生成桌面配置目录。")?,
        },
        patch(
            paths,
            FileKind::ClaudeThreepConfig,
            json!({"deploymentMode":"3p"}),
        )?,
        patch(
            paths,
            FileKind::ClaudeDesktopConfig,
            json!({"deploymentMode":"3p"}),
        )?,
    ])
}

pub fn supported_model_id(id: &str) -> bool {
    let normalized = id.to_ascii_lowercase();
    let id = normalized.strip_prefix("anthropic/").unwrap_or(&normalized);
    [
        "claude-sonnet-",
        "claude-opus-",
        "claude-haiku-",
        "claude-fable-",
    ]
    .iter()
    .any(|prefix| {
        id.strip_prefix(prefix)
            .is_some_and(|tail| !tail.is_empty() && !tail.contains('['))
    })
}
