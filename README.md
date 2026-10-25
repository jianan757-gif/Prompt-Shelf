# Prompt Shelf

Windows 优先的图文提示词快捷调用工具技术原型，采用 **Tauri 2 + Rust + React/TypeScript**。

**版本 0.1.0：4.3 Windows 核心技术 PoC。** 用户已确认阶段验证完成；本仓库用于公开技术审查，不是正式产品，尚未进入 4.4。macOS 仅预留适配接口，未实现系统粘贴。

## 审查入口

- [技术验收与交接报告](docs/public/technical-handoff.md)：验收结果、模块职责、证据边界、遗留风险。
- [测试证据说明](docs/validation/README.md)：已公开测试日志与未验证项。
- [任务书](Prompt%20Shelf%204.2%C2%B7v2.0/PromptShelf_4.2_Tauri单主线技术选型与PoC任务书_v2.0.md)及 [V4.11 定稿原型](Prompt%20Shelf%204.2%C2%B7v2.0/demos/01_V4.11_素材管理UI冻结基线.html)。早期快捷面板样机仅供历史参考，冲突时以定稿与当前已确认交互为准。
- [更新记录](CHANGELOG.md)、[贡献说明](CONTRIBUTING.md)、[MIT 许可证](LICENSE)。

请优先审查焦点恢复与误贴边界、队列取消、窗口生命周期及证据充分性。不要将编译或模拟测试通过视为所有真实应用兼容性通过。

## 已实现与边界

- Ctrl+Alt+1/2/3 呼出快捷面板，记录外部目标窗口并恢复焦点。
- 全文、效果图、参考图及多图顺序粘贴；显示已触发进度。
- 面板常驻，保留搜索和卡片位置；忙时 Esc 取消，空闲时 Esc 收起，隐藏后释放全局 Esc。
- 560 默认宽度、圆形半透明图片翻页按钮；主库按需创建及销毁。
- 100 案例/500 图片合成样本生成、搜索与窗口响应埋点、资源采样工具。

尚未实现正式素材 CRUD、数据库持久化、回收站、备份和安装升级。目标软件可能选择不同剪贴板格式或转码；进度代表已派发，不代表上传完成。完整逐应用兼容性矩阵、资源基线及性能阈值仍缺专项记录。

## 本地构建与启动（Windows）

需要 Windows、WebView2、Rust MSVC、Visual Studio C++ Build Tools、Node.js（支持 `--experimental-strip-types`）、pnpm；生成样本另需 Python 与 Pillow。依赖版本以两个 lockfile 为准。

```powershell
git clone https://github.com/jianan757-gif/Prompt-Shelf.git
cd Prompt-Shelf
python -m pip install Pillow
python scripts/generate-fixtures.py
cd poc
pnpm install --frozen-lockfile
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'src-tauri\target-round7'
pnpm tauri build --no-bundle -- --locked
cd ..
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\scripts\start-poc.ps1" -Fixture synthetic
```

样本生成器拒绝覆盖已有 `fixtures/100x500`。ExecutionPolicy Bypass 仅用于该 PowerShell 进程。构建输出在 `poc/src-tauri/target-round7/release/`，不生成安装器。round7 是内部修复批次，发布版本为 0.1.0。

首次启动默认隐藏：在专用目标输入框内按 Ctrl+Alt+1 呼出面板，点击提示词或图片。更换目标时先点新目标输入框，再按热键重新绑定。托盘可打开主库或退出；启动脚本拒绝同时运行同项目多版本。私人混合样本不随仓库提供，请先使用 synthetic。

## 验证命令

供需要复现的审查者使用，本次公开整理没有重跑已完成测试。

```powershell
cd poc
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

测试包含模拟平台与数据格式检查；Windows 目标应用实际收件需人工验证。原有运行日志默认保存在应用本地数据目录的 `logs/poc.jsonl`，其中可能包含本机路径和窗口身份，发布问题时请脱敏。

## 目录

| 路径 | 内容 |
|---|---|
| `poc/src` | React UI、搜索、轮播和会话逻辑 |
| `poc/src-tauri/src` | 窗口、队列、Esc 与平台适配器 |
| `scripts` | 样本生成、启动、校验及资源采样 |
| `tests/receiver` | 本地 paste 接收页，不代表真实应用验收 |
| `docs/public` | 公开技术交接报告 |
| `docs/validation` | 已有验证记录的公开副本 |
| `Prompt Shelf 4.2·v2.0` | 任务书与 HTML 原型 |

依赖、编译产物、私人截图、完整本机日志、历史源码 ZIP 和机器环境清单均不纳入版本控制。源码以 MIT 许可提供；第三方依赖适用其各自许可证。