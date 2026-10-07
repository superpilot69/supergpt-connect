use crate::{
    models::ValidatedInput,
    provider_fields,
    storage::{self, Change, FileKind, Paths},
};
use jsonc_parser::{
    cst::{CstInputValue, CstRootNode},
    ParseOptions,
};
use serde_json::{json, Value};

fn cst(value: Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(v) => CstInputValue::Bool(v),
        Value::Number(v) => CstInputValue::Number(v.to_string()),
        Value::String(v) => CstInputValue::String(v),
        Value::Array(v) => CstInputValue::Array(v.into_iter().map(cst).collect()),
        Value::Object(v) => {
            CstInputValue::Object(v.into_iter().map(|(k, v)| (k, cst(v))).collect())
        }
    }
}

pub fn change(paths: &Paths, input: &ValidatedInput) -> Result<Change, String> {
    let kind = FileKind::ClaudeVscode;
    let before = storage::read_optional(&paths.target(kind))?;
    let text = before
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("{}");
    // VS Code supports JSON + comments/trailing commas, not JSON5.
    let options = ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    };
    let doc: Value = jsonc_parser::parse_to_serde_value(text, &options)
        .map_err(|_| "VS Code 用户设置不是有效 JSONC，未修改配置。")?;
    if !doc.is_object() {
        return Err("VS Code 用户设置必须是对象，未修改配置。".into());
    }
    let key = "claudeCode.environmentVariables";
    let mut vars = match doc.get(key) {
        None => vec![],
        Some(Value::Array(values)) => values.clone(),
        _ => return Err("claudeCode.environmentVariables 必须是数组，未修改配置。".into()),
    };
    if vars.iter().any(|v| {
        v.get("name").and_then(Value::as_str).is_none()
            || v.get("value").and_then(Value::as_str).is_none()
    }) {
        return Err("Claude VS Code 环境变量格式无效，未修改配置。".into());
    }
    vars.retain(|v| !provider_fields::claude_floor_env(v["name"].as_str().unwrap()));
    for (name, value) in [
        ("ANTHROPIC_BASE_URL", input.root.as_str()),
        ("ANTHROPIC_AUTH_TOKEN", input.api_key.as_str()),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
    ] {
        vars.retain(|v| v["name"] != name);
        vars.push(json!({"name": name, "value": value}));
    }
    // CST replaces only the extension's property; all surrounding comments and
    // settings stay byte-for-byte intact, including trailing commas and URLs.
    let root = CstRootNode::parse(text, &options).map_err(|_| "无法解析 VS Code 设置。")?;
    let obj = root.object_value_or_set();
    if obj
        .properties()
        .iter()
        .filter(|prop| prop.decoded_name().as_deref() == Some(key))
        .count()
        > 1
    {
        return Err(
            "VS Code 中有重复的 claudeCode.environmentVariables 设置，请合并后再导入。".into(),
        );
    }
    if let Some(property) = obj.get(key) {
        property.set_value(cst(json!(vars)));
    } else {
        obj.append(key, cst(json!(vars)));
    }
    Ok(Change {
        kind,
        before,
        after: root.to_string(),
    })
}
