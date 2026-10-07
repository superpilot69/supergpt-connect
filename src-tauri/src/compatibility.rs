use crate::{models::Client, storage::Paths};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    pub installed: bool,
    pub version: Option<String>,
    pub warnings: Vec<String>,
    pub blocked: Option<String>,
}

fn output(program: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Version probes (including npm .cmd shims) must not flash a console window.
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let out = child.wait_with_output().ok()?;
                if out.stdout.len() > 8192 {
                    return None;
                }
                return String::from_utf8(out.stdout)
                    .ok()
                    .map(|v| v.trim().to_string());
            }
            Ok(None) if start.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn executable(paths: &Paths, name: &str) -> Option<PathBuf> {
    let mut roots: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    roots.push(paths.home.join(".local/bin"));
    #[cfg(not(windows))]
    roots.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    #[cfg(windows)]
    {
        roots.push(
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| paths.home.join("AppData/Roaming"))
                .join("npm"),
        );
        roots.push(
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| paths.home.join("AppData/Local"))
                .join("Microsoft/WinGet/Links"),
        );
    }
    find_executable(&roots, name, cfg!(windows))
}

fn find_executable(roots: &[PathBuf], name: &str, windows: bool) -> Option<PathBuf> {
    // npm also creates extensionless POSIX scripts on Windows; those cannot be launched there.
    let extensions: &[&str] = if windows {
        &["exe", "com", "cmd", "bat"]
    } else {
        &[""]
    };
    roots
        .iter()
        .filter(|p| p.is_absolute())
        .flat_map(|p| {
            extensions.iter().map(move |ext| {
                if ext.is_empty() {
                    p.join(name)
                } else {
                    p.join(format!("{name}.{ext}"))
                }
            })
        })
        .find(|p| p.is_file())
}

#[cfg(windows)]
fn windows_managed_provider() -> bool {
    use winreg::{enums::*, RegKey};
    // Claude gives machine policy precedence over user policy, even for app-only settings.
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if let Ok(key) = RegKey::predef(hive)
            .open_subkey_with_flags(r"SOFTWARE\Policies\Claude", KEY_READ | KEY_WOW64_64KEY)
        {
            let names: Vec<_> = key
                .enum_values()
                .filter_map(Result::ok)
                .filter(|(_, value)| matches!(value.vtype, REG_SZ | REG_EXPAND_SZ | REG_DWORD))
                .map(|(name, _)| name)
                .collect();
            if !names.is_empty() {
                return names
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case("inferenceProvider"));
            }
        }
    }
    false
}

fn version_number(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()) && part.contains('.'))
        .map(str::to_owned)
}

#[cfg(target_os = "macos")]
fn app(paths: &Paths, names: &[&str]) -> Option<PathBuf> {
    [
        PathBuf::from("/Applications"),
        paths.home.join("Applications"),
    ]
    .into_iter()
    .flat_map(|root| names.iter().map(move |name| root.join(name)))
    .find(|p| {
        if !p.is_dir() {
            return false;
        }
        // Some builds display "ChatGPT" while retaining the Codex bundle ID.
        // A regular ChatGPT-only app must not be reported as installed Codex.
        if p.file_name().and_then(|s| s.to_str()) == Some("ChatGPT.app") {
            return output(
                Path::new("/usr/libexec/PlistBuddy"),
                &[
                    "-c",
                    "Print :CFBundleIdentifier",
                    p.join("Contents/Info.plist").to_str().unwrap_or(""),
                ],
            )
            .as_deref()
                == Some("com.openai.codex");
        }
        true
    })
}

pub fn inspect(paths: &Paths, client: Client) -> Compatibility {
    if paths.sandbox {
        return Compatibility {
            installed: true,
            warnings: vec!["隔离测试：配置不会写入真实用户目录。".into()],
            ..Default::default()
        };
    }
    let mut result = Compatibility::default();
    match client {
        Client::Claude | Client::Codex => {
            if let Some(path) = executable(
                paths,
                if client == Client::Claude {
                    "claude"
                } else {
                    "codex"
                },
            ) {
                result.installed = true;
                result.version = output(&path, &["--version"])
                    .as_deref()
                    .and_then(version_number);
            }
            if client == Client::Claude
                && result
                    .version
                    .as_deref()
                    .is_some_and(|v| v.starts_with("1."))
            {
                result.warnings.push("检测到旧版 Claude Code 1.x：可读取中转站和默认模型，但不支持所选模型菜单。请升级 Claude Code 后再使用多模型选择。".into());
            }
        }
        Client::ClaudeVscode => {
            if let Ok(entries) = fs::read_dir(paths.home.join(".vscode/extensions")) {
                for entry in entries.flatten() {
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("anthropic.claude-code-")
                    {
                        result.installed = true;
                        if let Ok(text) = fs::read_to_string(entry.path().join("package.json")) {
                            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                                result.version = value["version"].as_str().map(str::to_owned);
                            }
                        }
                    }
                }
            }
            result.warnings.push("写入 VS Code 默认用户配置，并同步 Claude Code 模型设置。自定义 VS Code Profile、Remote SSH 和 WSL 需要在对应环境配置。".into());
        }
        Client::ClaudeDesktop | Client::CodexDesktop => {
            #[cfg(target_os = "macos")]
            if let Some(path) = app(
                paths,
                if client == Client::ClaudeDesktop {
                    &["Claude.app"]
                } else {
                    &["Codex.app", "ChatGPT.app"]
                },
            ) {
                result.installed = true;
                result.version = output(
                    Path::new("/usr/libexec/PlistBuddy"),
                    &[
                        "-c",
                        "Print :CFBundleShortVersionString",
                        path.join("Contents/Info.plist").to_str().unwrap_or(""),
                    ],
                );
            }
            #[cfg(not(target_os = "macos"))]
            {
                result.installed = if client == Client::ClaudeDesktop {
                    paths.claude_desktop.is_dir()
                } else {
                    paths.codex.is_dir()
                };
                result
                    .warnings
                    .push("已定位配置目录；此系统上的客户端版本需手动确认。".into());
            }
            if client == Client::ClaudeDesktop {
                #[cfg(windows)]
                if windows_managed_provider() {
                    result.blocked = Some("Claude 桌面版的推理服务由组织管理，本地导入不会生效。请由管理员调整网关配置。".into());
                }
                #[cfg(target_os = "macos")]
                {
                    let user = paths
                        .home
                        .file_name()
                        .and_then(|v| v.to_str())
                        .unwrap_or("");
                    for managed in [
                        PathBuf::from(
                            "/Library/Managed Preferences/com.anthropic.claudefordesktop.plist",
                        ),
                        PathBuf::from("/Library/Managed Preferences")
                            .join(user)
                            .join("com.anthropic.claudefordesktop.plist"),
                    ] {
                        if managed.is_file()
                            && output(
                                Path::new("/usr/libexec/PlistBuddy"),
                                &[
                                    "-c",
                                    "Print :inferenceProvider",
                                    managed.to_str().unwrap_or(""),
                                ],
                            )
                            .is_some()
                        {
                            result.blocked = Some("Claude 桌面版的推理服务由组织管理，本地导入不会生效。请由管理员调整网关配置。".into());
                        }
                    }
                }
            }
        }
    }
    if !result.installed {
        result.warnings.insert(
            0,
            format!(
                "未在常用位置检测到{}。可以先保存配置；安装或打开支持此配置的版本后才能验证生效。",
                client.name()
            ),
        );
    }
    if client.is_codex() {
        result
            .warnings
            .push("Codex 桌面版和终端版共用配置；导入与恢复会同时影响两者的本地会话。".into());
    }
    result
}

pub fn next_steps(client: Client) -> Vec<String> {
    match client {
        Client::ClaudeDesktop => vec![
            if cfg!(windows) {
                "从系统托盘退出 Claude，再重新打开；仅关闭窗口可能仍在后台运行。".into()
            } else {
                "完全退出 Claude（macOS 使用 ⌘Q），再打开桌面应用。".into()
            },
            "进入第三方推理模式；若显示部署方式选择器，请选第三方服务。Chat、Cowork 和本地 Code 使用同一中转站。".into(),
            "新建会话，确认模型菜单后发送测试消息。原 Claude.ai 云端聊天仍保留在原模式中。".into(),
        ],
        Client::Claude => vec!["退出当前 Claude Code 进程，在新终端重新运行 claude。".into(), "输入 /status，确认 API 地址；使用 /model 检查模型。旧版请先升级。".into()],
        Client::ClaudeVscode => vec!["在 VS Code 执行 Developer: Reload Window，再新建 Claude Code 会话。".into(), "确认使用默认 VS Code Profile；工作区设置可能覆盖用户设置。".into()],
        Client::CodexDesktop => vec!["完全退出 Codex 桌面应用，再重新打开，新建本地会话。".into(), "确认使用 API Key 登录方式、SuperGPT Connect 服务和所选模型；已有会话或云端任务不会自动迁移。".into()],
        Client::Codex => vec!["退出当前 Codex 进程并重新运行 codex。".into(), "输入 /status，确认 SuperGPT Connect 服务和所选模型。桌面版也需重启后生效。".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_resolves_native_and_npm_launchers_without_picking_posix_shim() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("中文用户 Test/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("claude"), "#!/bin/sh").unwrap();
        fs::write(bin.join("claude.ps1"), "unused").unwrap();
        let roots = [bin.clone()];
        assert_eq!(find_executable(&roots, "claude", true), None);
        fs::write(bin.join("claude.cmd"), "@echo off").unwrap();
        assert_eq!(
            find_executable(&roots, "claude", true),
            Some(bin.join("claude.cmd"))
        );
        fs::write(bin.join("claude.exe"), "fixture").unwrap();
        assert_eq!(
            find_executable(&roots, "claude", true),
            Some(bin.join("claude.exe"))
        );
        assert_eq!(
            find_executable(&roots, "claude", false),
            Some(bin.join("claude"))
        );
    }

    #[test]
    fn launcher_search_preserves_path_order_and_ignores_relative_roots() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        for dir in [&first, &second] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(first.join("codex.cmd"), "fixture").unwrap();
        fs::write(second.join("codex.exe"), "fixture").unwrap();
        assert_eq!(
            find_executable(&[PathBuf::from("."), first.clone(), second], "codex", true),
            Some(first.join("codex.cmd"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_can_probe_cmd_with_spaces_and_unicode_in_path() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("中文 User/npm");
        fs::create_dir_all(&dir).unwrap();
        let script = dir.join("claude.cmd");
        fs::write(&script, "@echo off\r\necho 2.0.0 (Claude Code)\r\n").unwrap();
        assert_eq!(
            output(&script, &["--version"])
                .as_deref()
                .and_then(version_number)
                .as_deref(),
            Some("2.0.0")
        );
    }
}
