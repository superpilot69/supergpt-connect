<p align="center"><img src="public/supergpt.png" alt="SuperGPT Connect" width="96"></p>

# SuperGPT Connect

一个简单的本地 API 配置器。选择中转站、选择 Claude 或 Codex、填写密钥、勾选模型，然后导入。

**v0.1.0 · 首个正式公开版本** · [下载](https://github.com/superpilot69/supergpt-connect/releases/tag/v0.1.0) · [反馈问题](https://github.com/superpilot69/supergpt-connect/issues) · [MIT 许可](LICENSE)

## 下载与使用

| 系统 | 下载 | 打开方式 |
| --- | --- | --- |
| Windows 10 / 11 · x64 | [Windows 免安装 ZIP](https://github.com/superpilot69/supergpt-connect/releases/download/v0.1.0/SuperGPT-Connect-0.1.0-Windows-x64-portable.zip) | 全部解压，双击 `SuperGPT Connect.exe` |
| macOS · Apple Silicon | [Mac ZIP](https://github.com/superpilot69/supergpt-connect/releases/download/v0.1.0/SuperGPT-Connect-0.1.0-macOS-arm64.zip) | 解压后打开 `.app`，也可拖入「应用程序」 |

官网与本页的下载均直接使用 [GitHub Releases](https://github.com/superpilot69/supergpt-connect/releases/tag/v0.1.0)。[SHA-256 校验值](https://github.com/superpilot69/supergpt-connect/releases/download/v0.1.0/SHA256SUMS.txt) 也在同一版本下。Mac 包仅支持 M 系列芯片，不支持 Intel Mac。

**Windows 与 macOS 均已通过用户实机测试。** Windows 验证于 2026-10-07 确认，详细范围见 [验证记录](VERIFICATION.md)。

运行时无需安装 Node.js 或 Rust。Windows 使用系统 WebView2 Runtime；缺少时，按应用提示从 [微软官网](https://developer.microsoft.com/microsoft-edge/webview2/) 安装。Windows 包未进行发布者签名；Mac 包使用 ad-hoc 签名，尚未完成 Apple Developer ID 签名与公证。校验值见 Release 的 `SHA256SUMS.txt`。

### 下载提示“不是常下载的文件”

v0.1.0 已收到 Chrome 以“不是常下载的文件，可能具有危险性”拦截 Mac ZIP 的反馈。这属于 Chrome 的“不常见文件”提示；仅凭这句话不能判断文件已被检测为恶意软件，也不能保证它安全。详见 [Google 对下载拦截的说明](https://support.google.com/chrome/answer/6261569?hl=zh-Hans)。

Cloudflare 与 GitHub 提供的包及校验值一致。更换下载来源不保证消除提示，SHA-256 一致也不等于安全扫描通过。当前 Mac 包的完整性检查通过，但未获 Developer ID 签名和 Apple 公证，Gatekeeper 评估仍拒绝运行。正式签名与公证还未完成；即使完成，Chrome 的下载信誉也需独立验证。

请保留浏览器保护设置。若无法接受当前发布包的信任状态，可暂缓安装，或审阅源码后自行构建；遇到不同的拦截说明，请通过 Issues 提供原文，不要附上 API Key。

### 配置步骤

1. **选择中转站**：默认 [SuperGPT](https://console.supergpt.dev)，也可以添加自己的站点。一次只使用一个。
2. **选择客户端**：选择 Claude 或 Codex，再选择桌面版、终端版或 VS Code 扩展。
3. **输入 API Key**：点击获取可用模型，读取站点的 `/v1/models`。
4. **勾选模型并导入**：选择需要的模型，指定默认模型，点击导入。

导入后请完全退出目标客户端，再重新打开并新建会话。Claude 在 Windows 上可能留在系统托盘，需要从托盘菜单退出。

需要撤回时，点击右上角 **恢复原配置**，选择客户端，再点恢复。多个站点之间反复导入时，仍保留第一次导入前的恢复点。

## 支持的客户端

| 客户端 | 配置方式 |
| --- | --- |
| Claude 桌面版 | 第三方推理模式的独立网关 profile，用于 Chat、Cowork 与本地 Code |
| Claude Code 终端版 | 用户 `settings.json` 中的 API、默认模型与可用模型菜单 |
| Claude Code · VS Code | 默认用户的扩展设置，并同步 Claude Code 的模型设置 |
| Codex 桌面版与终端版 | 共享用户配置、API Key 认证与所选模型目录 |

尊重 `CLAUDE_CONFIG_DIR` 与 `CODEX_HOME`。Windows 版配置 Windows 本机客户端；WSL、Remote SSH、自定义 VS Code Profile 需要在对应环境配置。

适配范围是支持本地 API 配置的客户端版本，不包含 Claude 网页版、手机端及云端任务。Claude Code 1.x 不支持新版自定义模型菜单；Claude 桌面版需要支持第三方推理模式。受组织管理的 Claude provider、已启用顶层自定义 profile 的 Codex 会阻止本地导入。

中转站须提供 Bearer 认证的 `/v1/models`，并支持目标客户端的 Messages 或 Responses 协议。**读取模型列表、写入配置不等于已验证对话与工具调用。** 导入后需在目标客户端实际确认。测试范围见 [VERIFICATION.md](VERIFICATION.md)。

## 配置与数据

- API Key 不进入站点列表、浏览器存储或日志。获取目录后，仅在当前进程内保存，导入成功后清空；导入时会写入所选客户端需要的本地认证配置。
- 保留 MCP、权限、hooks、无关配置，以及支持范围内的 TOML / JSONC 注释。
- 写入前自动备份，使用文件锁、原子替换与中断恢复，避免部分配置被覆盖。
- 导入后若又修改了配置，恢复前会要求确认，同时保留当前内容的副本。
- 没有恢复记录时不会重置客户端。恢复的是本机文件，不能让服务端失效的登录令牌重新有效。
- 恢复记录位于用户目录的 `.supergpt-connect`，可能含有原凭据，请不要公开上传。macOS / Linux 新写入的配置文件权限为 `0600`。

Codex 的外部模型目录会替换内置目录。网关提供完整能力元数据时保留所选项；仅提供模型名称时使用保守模板，不能据此推断实际上下文长度、视觉或工具能力。

## 开发与构建

需要 Node.js 22+（推荐 24）、pnpm 10 与当前稳定版 Rust。macOS 需要 Xcode Command Line Tools；Windows 需要 MSVC Build Tools 和 Windows SDK。

```sh
pnpm install --frozen-lockfile
pnpm dev
pnpm check
pnpm test:ui
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked
```

`pnpm dev:web` 只预览界面，不会修改本机配置。打包：

```sh
# macOS Apple Silicon ZIP（在 macOS 执行）
rustup target add aarch64-apple-darwin
pnpm build:macos

# Windows x64 免安装 ZIP（在 Windows 执行）
pnpm build:windows
```

产物在 `release/`。Windows 脚本跳过安装器，校验 EXE 架构、版本、启动权限与 DLL 依赖；CRT 和 WebView2Loader 静态链接，不要求用户另装 VC++ Redistributable。Mac 脚本验证应用版本、arm64 架构与 ad-hoc 签名。

也可在 macOS 交叉构建 Windows 包：

```sh
brew install llvm lld
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
PATH="$(brew --prefix llvm)/bin:$PATH" XWIN_ARCH=x86_64 pnpm build:windows
```

`.cargo/config.toml` 控制完整静态 CRT 链接；`tauri.windows.conf.json` 关闭 Tauri 自身的混合 CRT 覆盖以避免冲突。

可选 GitHub Actions 模板位于 [ci/build.yml](ci/build.yml)。复制到 `.github/workflows/build.yml` 后，可在 Mac / Windows runner 上执行检查、测试、构建并保存 ZIP 产物；本次发布尚未启用该流程。

## 隔离测试

Rust 测试使用临时用户目录和虚构密钥。原生 debug 界面可使用独立测试目录：

```sh
python3 scripts/mock-gateway.py
# 在另一终端中，使用已构建的 debug 应用：
SUPERGPT_CONNECT_TEST_HOME=/absolute/path/to/test-home src-tauri/target/debug/supergpt-connect
```

添加站点 `http://127.0.0.1:18473`，输入虚构 Key `sk-local-fixture-only`。模拟网关只返回模型目录，不调用真实模型。测试目录覆盖仅在 debug 构建生效，界面会显示测试模式。

反馈问题时请说明系统、客户端版本与操作步骤；不要上传 API Key、认证文件或完整恢复记录。

## 开源与来源

采用 MIT 许可，基于 [CC Switch](https://github.com/farion1231/cc-switch) 裁剪配置引擎，保留上游版权与许可。使用 React、Tauri 和 Rust；来源与裁剪范围见 [UPSTREAM.md](UPSTREAM.md)，第三方许可见 [licenses/](licenses/)。本项目独立维护，不是 Claude 或 Codex 的官方客户端。
