# v0.1.0 验证记录

验证日期：2026-10-07。v0.1.0 是首个正式公开版本；此前的内部迭代不作为公开 Release。

## 自动测试

- macOS / Apple Silicon：54 项 Rust 配置测试、10 项前端状态测试通过；TypeScript 检查和前端生产构建通过。
- 配置测试覆盖多次导入后保留原始恢复点、已有认证文件精确还原、新建文件撤回、注释保留、过期预览拒绝、后续编辑确认、中断恢复、损坏配置、文件锁、中文及空格路径。
- 模型发现测试使用回环地址模拟网关与虚构密钥，检查认证、重定向拒绝、模型选择与获取结果绑定，不调用真实推理接口。
- Windows 的原生配置测试代码包括 `.cmd` 启动器与 AppData 路径检查；交叉编译通过不代表已在 Windows 执行。

## 原生与产物检查

- macOS 界面在先前相同功能版本中完成隔离导入、恢复、深浅主题及小窗口检查；用户也已自行测试 Mac 版本。公开版统一版本标识和打包方式。
- Windows x64 EXE 检查覆盖 PE 架构、GUI 子系统、0.1.0 版本资源、应用图标、`asInvoker` 权限与系统 DLL 导入。无需外置 VC++ 或 WebView2Loader DLL；实际界面仍需系统 WebView2 Runtime。
- Mac 包检查 arm64、0.1.0 应用版本与 ad-hoc 签名；两平台 ZIP 校验 CRC、归档程序与构建产物的一致性，并提供 SHA-256。

## 用户实机反馈与验证范围

用户于 2026-10-07 确认 Windows 版已在自己的电脑验证通过；此前也已确认 Mac 版测试通过。这是用户的实机反馈，不是 Windows CI 测试记录。

缺少 WebView2、组织策略管控、所有客户端版本及所有模型工具调用组合仍未逐项实机验证。支持配置格式不等于覆盖任意客户端历史版本、所有中转站协议或模型能力。

Windows 发布包尚未签名。Mac 发布包尚未完成 Apple Developer ID 签名和公证。测试与打包过程不修改真实 Claude / Codex 配置。

## 2026-10-07 下载信誉复核

- 用户在 Chrome 下载 Mac ZIP 时收到：“Chrome 阻止了此项下载操作，因为这不是常下载的文件，可能具有危险性”。这是已复现的下载阻碍，不能用此前下载成功或功能测试通过来代替验证。
- 对实际发布的 Mac ZIP 解包复核：归档未加密，CRC 正常；SHA-256 为 `41e73fc31c2fb762ff76f7cb1fecc6d24a721f3151220fa90293635a14ca752f`，与 Cloudflare 和 GitHub 发布记录一致。
- 包内应用 `codesign --verify --deep --strict` 通过，但签名为 ad-hoc，未绑定开发者 Team ID；`spctl --assess --type execute` 返回拒绝。该检查仅评估签名与系统信任，不是独立恶意软件扫描。
- 构建机当前没有可用的代码签名身份，尚不能完成 Developer ID 签名与 Apple 公证。Chrome Safe Browsing 与 macOS Gatekeeper 是独立检查，完成公证后仍须验证 Chrome 实际下载行为。
