# CC Switch 代码来源

- 项目：https://github.com/farion1231/cc-switch
- 固定提交：`8596a233b2373226a0307d5f80ec0954d5fa162f`
- 上游 package.json 版本：4.0.3
- 原作者：Jason Young
- 许可：MIT，完整声明见 `licenses/CC-Switch-MIT.txt`；应用包内也附带此文件。

本项目提取 CC Switch 的配置修改引擎，保留 React + Tauri 技术栈，重写成四步连接器。

| 本项目 | 上游路径 | 调整 |
| --- | --- | --- |
| `src-tauri/src/patch/json.rs` | `src-tauri/src/live/patch/json.rs` | 保留 JSON 补丁与格式处理，调整测试模块路径 |
| `src-tauri/src/patch/toml.rs` | `src-tauri/src/live/patch/toml.rs` | 保留 TOML 注释与结构补丁，调整测试模块路径 |
| `src-tauri/src/patch/mod.rs` | `src-tauri/src/live/patch/mod.rs` | 移除数据库、整文件引擎及 AppError 依赖，保留补丁接口与解析错误 |
| `src-tauri/src/provider_fields.rs` | `src-tauri/src/live/floor.rs` | 提取 Claude 和 Codex 的服务商字段定义 |
| `src-tauri/src/resources/codex_native_responses_template.json` | 同路径 | 复制 Responses 目录基础模板；名称目录使用保守能力参数，网关元数据优先 |
| `src-tauri/src/adapter.rs` 的 Claude 模型菜单 | `src-tauri/src/mode/controller.rs` | 参考上游 `modelPicker` 自定义选项结构，按用户勾选结果写入 |
| `src-tauri/src/desktop_config.rs` | `src-tauri/src/claude_desktop_config.rs` | 采用 3P profile + metadata + deploymentMode 布局；使用独立 profile ID，不复制代理服务和放宽网络权限的设置 |

新写的代码负责单页 UI、模型目录读取、客户端适配、备份事务和恢复。存储采用与上游相同的同目录临时文件原子替换策略，但使用 tempfile 实现，未复制上游 SQLite、代理和账户系统。

未包含上游的代理转发、自动故障切换、统计、MCP/Skills 管理、订阅登录、云同步、托盘、自动更新或深链接处理器。没有继承上游更新地址或应用标识，也不读取 CC Switch 数据库。


品牌资源：应用界面与打包图标使用 SuperGPT 的 S 标志，配色来自 [SuperGPT 控制台](https://console.supergpt.dev)。

VS Code 注释保留使用 `jsonc-parser` 0.34.0（MIT），许可证随包附带于 `licenses/jsonc-parser-MIT.txt`。
