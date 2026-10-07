//! Restore previews expose file names and timestamps, never saved credentials.
use crate::{
    models::Client,
    storage::{self, Backup, FileKind, Paths},
};
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const CLIENTS: [Client; 4] = [
    Client::ClaudeDesktop,
    Client::Claude,
    Client::ClaudeVscode,
    Client::Codex,
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreFile {
    pub label: &'static str,
    pub path: String,
    pub existed_before: bool,
    pub modified_since_import: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePoint {
    pub client: Client,
    pub available: bool,
    pub pending: bool,
    pub created_at: Option<u64>,
    pub files: Vec<RestoreFile>,
    pub review_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub client: Client,
    pub restored_files: usize,
    pub safety_backup_path: String,
    pub was_pending: bool,
}

struct Plan {
    record_path: PathBuf,
    record_text: String,
    backup: Backup,
    current: Vec<Option<String>>,
    point: RestorePoint,
}

fn label(kind: FileKind) -> &'static str {
    match kind {
        FileKind::ClaudeSettings => "Claude Code 的 API、模型及原有设置",
        FileKind::ClaudeDesktopConfig => "Claude 桌面版原有模式与偏好",
        FileKind::ClaudeThreepConfig => "Claude 第三方模式设置",
        FileKind::ClaudeDesktopProfile => "Claude 桌面版网关、密钥与模型",
        FileKind::ClaudeDesktopMeta => "Claude 桌面版启用的网关",
        FileKind::ClaudeVscode => "VS Code 原有用户设置",
        FileKind::CodexConfig => "Codex 原有服务商、模型与设置",
        FileKind::CodexAuth => "Codex 原有登录或 API 凭据",
        FileKind::CodexCatalog => "Codex 模型目录",
    }
}

fn plan(paths: &Paths, client: Client) -> Result<Plan, String> {
    let client = client.backup_client();
    let pending = paths.pending(client);
    let record_path = paths.record(client, pending);
    let (record_text, backup) = storage::read_backup(paths, client, &record_path)?;
    let mut current = vec![];
    let mut files = vec![];
    for entry in &backup.entries {
        let content = storage::read_optional(&paths.target(entry.kind))?;
        let modified = content != entry.before
            && content.as_deref().map(storage::digest).as_deref() != Some(&entry.after_hash);
        files.push(RestoreFile {
            label: label(entry.kind),
            path: entry.path.to_string_lossy().into_owned(),
            existed_before: entry.before.is_some(),
            modified_since_import: modified,
        });
        current.push(content);
    }
    // Bind the confirmation to both the backup and the current file bytes. If a
    // client/editor writes again while the user reviews, require a fresh preview.
    let fingerprint = serde_json::to_string(&(
        storage::digest(&record_text),
        current
            .iter()
            .map(|v| v.as_deref().map(storage::digest))
            .collect::<Vec<_>>(),
    ))
    .map_err(|_| "无法生成恢复预览。")?;
    let point = RestorePoint {
        client,
        available: true,
        pending,
        created_at: backup
            .created_at
            .or_else(|| storage::record_time(&record_path)),
        files,
        review_id: Some(storage::digest(&fingerprint)),
        error: None,
    };
    Ok(Plan {
        record_path,
        record_text,
        backup,
        current,
        point,
    })
}

pub fn list(paths: &Paths) -> Vec<RestorePoint> {
    CLIENTS
        .into_iter()
        .map(|client| {
            if !paths.has_backup(client) {
                return RestorePoint {
                    client,
                    available: false,
                    pending: false,
                    created_at: None,
                    files: vec![],
                    review_id: None,
                    error: None,
                };
            }
            match plan(paths, client) {
                Ok(plan) => plan.point,
                Err(error) => RestorePoint {
                    client,
                    available: false,
                    pending: paths.pending(client),
                    created_at: None,
                    files: vec![],
                    review_id: None,
                    error: Some(error),
                },
            }
        })
        .collect()
}

#[derive(Serialize)]
struct SafetyFile<'a> {
    kind: FileKind,
    path: &'a std::path::Path,
    contents: &'a Option<String>,
}

pub fn apply(
    paths: &Paths,
    client: Client,
    review_id: &str,
    overwrite_modified: bool,
) -> Result<RestoreResult, String> {
    let _lock = paths.lock()?;
    let plan = plan(paths, client)?;
    if plan.point.review_id.as_deref() != Some(review_id) {
        return Err("备份或当前配置已经变化，请刷新预览后再恢复。".into());
    }
    if !overwrite_modified
        && plan
            .point
            .files
            .iter()
            .any(|file| file.modified_since_import)
    {
        return Err("导入后的配置有新改动。请确认备份当前配置后再恢复。".into());
    }
    let files: Vec<_> = plan
        .backup
        .entries
        .iter()
        .zip(&plan.current)
        .map(|(entry, contents)| SafetyFile {
            kind: entry.kind,
            path: &entry.path,
            contents,
        })
        .collect();
    let snapshot = serde_json::to_vec(&serde_json::json!({
        "version":1, "client":plan.point.client, "createdAt":storage::timestamp(), "files":files
    }))
    .map_err(|_| "无法备份当前配置，未执行恢复。")?;
    if snapshot.len() > 8 * 1024 * 1024 {
        return Err("当前配置备份超过大小限制，未执行恢复。".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let snapshot_path = paths
        .state
        .join("recovery")
        .join(format!("{}-{nonce}.json", plan.point.client.id()));
    storage::atomic_write(&snapshot_path, &snapshot)?;

    for (entry, expected) in plan.backup.entries.iter().zip(&plan.current) {
        if storage::read_optional(&entry.path)? != *expected {
            return Err("当前配置又发生变化，请关闭对应客户端并刷新预览。".into());
        }
    }
    let result = (|| {
        for (entry, expected) in plan.backup.entries.iter().zip(&plan.current).rev() {
            if storage::read_optional(&entry.path)? != *expected {
                return Err("恢复过程中检测到其他程序写入，操作已停止。".to_string());
            }
            if *expected == entry.before {
                continue;
            }
            match &entry.before {
                Some(original) => storage::atomic_write(&entry.path, original.as_bytes())?,
                None => fs::remove_file(&entry.path)
                    .map_err(|_| "无法撤回新建的配置文件。".to_string())?,
            }
        }
        for entry in &plan.backup.entries {
            if storage::read_optional(&entry.path)? != entry.before {
                return Err("恢复后的配置核对失败，请关闭客户端后重试。".into());
            }
        }
        if storage::read_optional(&plan.record_path)?.as_deref() != Some(&plan.record_text) {
            return Err("恢复记录发生变化，请刷新状态。".into());
        }
        fs::remove_file(&plan.record_path)
            .map_err(|_| "原配置已写回，但恢复记录未清理，请刷新后重试。".to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        return Err(format!(
            "{error} 恢复前的配置已保留在 {}。",
            snapshot_path.display()
        ));
    }
    Ok(RestoreResult {
        client: plan.point.client,
        restored_files: plan.backup.entries.len(),
        safety_backup_path: snapshot_path.to_string_lossy().into_owned(),
        was_pending: plan.point.pending,
    })
}
