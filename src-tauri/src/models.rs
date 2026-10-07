use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Claude,
    Codex,
    #[serde(rename = "claude-desktop")]
    ClaudeDesktop,
    #[serde(rename = "claude-vscode")]
    ClaudeVscode,
    #[serde(rename = "codex-desktop")]
    CodexDesktop,
}
impl Client {
    pub const ALL: [Self; 5] = [
        Self::CodexDesktop,
        Self::Codex,
        Self::ClaudeDesktop,
        Self::Claude,
        Self::ClaudeVscode,
    ];
    pub fn is_codex(self) -> bool {
        matches!(self, Self::Codex | Self::CodexDesktop)
    }
    // Both Codex entry points share one configuration and one restore point.
    pub fn backup_client(self) -> Self {
        if self == Self::CodexDesktop {
            Self::Codex
        } else {
            self
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::ClaudeDesktop => "claude-desktop",
            Self::ClaudeVscode => "claude-vscode",
            Self::CodexDesktop => "codex-desktop",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code 终端版",
            Self::Codex => "Codex 终端版",
            Self::ClaudeDesktop => "Claude 桌面版",
            Self::ClaudeVscode => "Claude Code · VS Code",
            Self::CodexDesktop => "Codex 桌面版",
        }
    }
}

// Values containing credentials deliberately do not implement Debug or Serialize.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryInput {
    pub client: Client,
    pub api_key: String,
    pub base_url: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionInput {
    pub discovery_id: String,
    pub selected_models: Vec<String>,
    pub default_model: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ModelChoice {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResponse {
    pub discovery_id: String,
    pub client: Client,
    pub base_url: String,
    pub models: Vec<ModelChoice>,
}

pub struct DiscoveredModels {
    pub client: Client,
    pub api_key: String,
    pub root: String,
    pub payload: Value,
    pub choices: Vec<ModelChoice>,
}
pub struct ValidatedInput {
    pub client: Client,
    pub api_key: String,
    pub root: String,
    pub model: String,
    pub selected_models: Vec<String>,
    pub catalog: Option<String>,
}
pub struct DiscoverySession {
    id: String,
    created: Instant,
    data: DiscoveredModels,
}
impl DiscoverySession {
    pub fn new(data: DiscoveredModels) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            id: format!("{now:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed)),
            created: Instant::now(),
            data,
        }
    }
    pub fn response(&self) -> DiscoveryResponse {
        DiscoveryResponse {
            discovery_id: self.id.clone(),
            client: self.data.client,
            base_url: self.data.root.clone(),
            models: self.data.choices.clone(),
        }
    }
    pub fn prepare(&self, input: &SelectionInput) -> Result<ValidatedInput, String> {
        if input.discovery_id != self.id || self.created.elapsed() > Duration::from_secs(15 * 60) {
            return Err("模型列表已失效，请重新获取后再导入。".into());
        }
        prepare_selection(&self.data, &input.selected_models, &input.default_model)
    }
}

pub fn normalize_root(input: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(input.trim())
        .map_err(|_| "API 地址格式不正确，请填写完整的 https:// 地址。")?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("API 地址需要使用 HTTPS；本机调试地址可以使用 HTTP。".into());
    }
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("API 地址不能包含账号、密码、查询参数或锚点。".into());
    }
    let path = url.path().trim_end_matches('/').to_string();
    if path.ends_with("/responses")
        || path.ends_with("/messages")
        || path.ends_with("/chat/completions")
    {
        return Err("请填写 API 基础地址，不要包含 /responses 或 /messages 等接口路径。".into());
    }
    url.set_path(path.strip_suffix("/v1").unwrap_or(&path));
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub fn catalog_models(payload: &Value) -> Vec<String> {
    let rows = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array);
    let mut models = Vec::new();
    if let Some(rows) = rows {
        for row in rows.iter().take(5000) {
            if let Some(id) = row
                .get("id")
                .or_else(|| row.get("slug"))
                .and_then(Value::as_str)
            {
                let id = id.trim();
                if !id.is_empty()
                    && id.len() <= 200
                    && !id.chars().any(char::is_control)
                    && !models.iter().any(|s| s == id)
                {
                    models.push(id.to_string());
                }
            }
        }
    }
    models
}

pub async fn discover(input: DiscoveryInput) -> Result<DiscoveredModels, String> {
    let api_key = input.api_key.trim().to_string();
    if api_key.is_empty()
        || api_key.len() > 4096
        || api_key.chars().any(char::is_whitespace)
        || api_key.chars().any(char::is_control)
    {
        return Err("请粘贴完整的 API 密钥，密钥中不能包含空格或换行。".into());
    }
    let root = normalize_root(&input.base_url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "无法初始化网络连接。")?;
    // The credential is only sent to this exact origin. No redirect or response body is echoed.
    let mut response = client
        .get(format!("{root}/v1/models"))
        .bearer_auth(&api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                "连接超时，请检查网络和 API 地址。"
            } else {
                "无法连接 API，请检查网络、证书和 API 地址。"
            }
            .to_string()
        })?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 => "密钥未通过验证，请检查是否复制完整。".into(),
            403 => "此密钥没有访问权限，请检查分组、有效期或账户状态。".into(),
            404 | 405 => {
                "此地址没有提供模型目录，请检查 API 基础地址。首版需要 /v1/models 接口。".into()
            }
            429 => "请求过于频繁或额度受限，请稍后重试。".into(),
            code => format!("模型目录暂时不可用（HTTP {code}），配置尚未写入。"),
        });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "读取模型目录失败，请重试。")?
    {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("模型目录过大，配置尚未写入。".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let payload: Value =
        serde_json::from_slice(&bytes).map_err(|_| "API 返回的模型目录不是有效 JSON。")?;
    let models = catalog_models(&payload);
    if models.is_empty() {
        return Err("密钥返回的模型目录为空，请检查分组是否有可用模型。".into());
    }
    let rows = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array);
    let choices = models
        .into_iter()
        .map(|id| {
            let label = rows
                .and_then(|rows| {
                    rows.iter().find(|row| {
                        row.get("id")
                            .or_else(|| row.get("slug"))
                            .and_then(Value::as_str)
                            .is_some_and(|v| v.trim() == id)
                    })
                })
                .and_then(|row| row.get("display_name").or_else(|| row.get("name")))
                .and_then(Value::as_str)
                .filter(|name| {
                    !name.is_empty() && name.len() <= 200 && !name.chars().any(char::is_control)
                })
                .unwrap_or(&id)
                .to_string();
            ModelChoice { id, name: label }
        })
        .collect();
    Ok(DiscoveredModels {
        client: input.client,
        api_key,
        root,
        payload,
        choices,
    })
}

pub fn prepare_selection(
    data: &DiscoveredModels,
    selected: &[String],
    default_model: &str,
) -> Result<ValidatedInput, String> {
    if selected.is_empty() || selected.len() > 200 {
        return Err("请勾选 1–200 个需要导入的模型。".into());
    }
    let available: HashSet<&str> = data.choices.iter().map(|m| m.id.as_str()).collect();
    let mut seen = HashSet::new();
    for id in selected {
        if !available.contains(id.as_str()) || !seen.insert(id) {
            return Err(
                "所选模型不在本次密钥返回的目录中，或存在重复选择。请重新获取模型。".into(),
            );
        }
    }
    if !selected.iter().any(|id| id == default_model) {
        return Err("请从已勾选模型中指定一个默认模型。".into());
    }
    let catalog = if data.client.is_codex() {
        Some(build_codex_catalog(&data.payload, selected)?)
    } else {
        None
    };
    Ok(ValidatedInput {
        client: data.client,
        api_key: data.api_key.clone(),
        root: data.root.clone(),
        model: default_model.into(),
        selected_models: selected.to_vec(),
        catalog,
    })
}

// CC Switch's native Responses catalog template; gateway-supplied metadata takes precedence.
// The selected list is the complete external catalog, so unchecked models never leak into it.
pub fn build_codex_catalog(payload: &Value, selected: &[String]) -> Result<String, String> {
    let template: Value = serde_json::from_str(include_str!(
        "resources/codex_native_responses_template.json"
    ))
    .map_err(|_| "模型目录模板不可用。")?;
    let source = payload.get("models").and_then(Value::as_array);
    let entries: Vec<Value> = selected
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut row = template.clone();
            // Names alone do not declare capabilities, including a slug-only native catalog.
            row["context_window"] = json!(32768);
            row["max_context_window"] = json!(32768);
            row["input_modalities"] = json!(["text"]);
            row["default_reasoning_level"] = Value::Null;
            row["supported_reasoning_levels"] = json!([]);
            row["supports_reasoning_summaries"] = json!(false);
            if let Some(original) = source
                .and_then(|rows| {
                    rows.iter()
                        .find(|m| m.get("slug").and_then(Value::as_str) == Some(id))
                })
                .and_then(Value::as_object)
            {
                for (key, value) in original {
                    row[key] = value.clone();
                }
                if !original.contains_key("max_context_window") {
                    row["max_context_window"] = row["context_window"].clone();
                }
            }
            row["slug"] = json!(id);
            if source.is_none() {
                row["display_name"] = json!(id);
                row["description"] = json!(id);
            } else if row["display_name"] == template["display_name"] {
                row["display_name"] = json!(id);
                row["description"] = json!(id);
            }
            row["visibility"] = json!("list");
            row["supported_in_api"] = json!(true);
            row["priority"] = json!(index);
            row["upgrade"] = Value::Null;
            row
        })
        .collect();
    serde_json::to_string_pretty(&json!({"models":entries}))
        .map_err(|_| "无法生成模型目录。".into())
}

#[cfg(test)]
mod session_tests {
    use super::*;
    #[test]
    fn expired_discovery_requires_a_new_fetch_even_with_the_matching_id() {
        let mut session = DiscoverySession::new(DiscoveredModels {
            client: Client::Claude,
            api_key: "fake-only".into(),
            root: "https://example.test".into(),
            payload: json!({}),
            choices: vec![ModelChoice {
                id: "a".into(),
                name: "a".into(),
            }],
        });
        let input = SelectionInput {
            discovery_id: session.response().discovery_id,
            selected_models: vec!["a".into()],
            default_model: "a".into(),
        };
        assert!(session.prepare(&input).is_ok());
        session.created = Instant::now() - Duration::from_secs(901);
        assert!(session.prepare(&input).is_err());
    }
}
