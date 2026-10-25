# Prompt Shelf

Windows 优先的图文提示词快捷调用工具技术原型，采用 **Tauri 2 + Rust + React/TypeScript**。

**已发布基线 v0.1.0：4.3 Windows PoC；当前工作区：4.4 阶段 A 数据与资产底座。** 本轮只接入 SQLite、共享原图、缩略图和单实例，主库仍为只读 UI，不进入 B1。macOS 仅预留适配接口，未实现系统粘贴。

## 审查入口

- [4.4 架构与实施计划](Prompt%20Shelf%204.4%20正式开发架构与实施计划.md)、[阶段 A 交付说明](docs/public/stage-a-delivery.md)。
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

阶段 A 已接入 SQLite 数据读取与显式离线样本导入。正式素材 CRUD UI、自动草稿、回收站、备份和安装升级仍留在后续阶段。目标软件可能选择不同剪贴板格式或转码；进度代表已派发，不代表上传完成。完整逐应用兼容性矩阵、资源基线及性能阈值仍缺专项记录。

## 本地构建与启动（Windows）

需要 Windows、WebView2、Rust MSVC、Visual Studio C++ Build Tools、Node.js（支持 `--experimental-strip-types`）、pnpm；生成样本另需 Python 与 Pillow。依赖版本以两个 lockfile 为准。

```powershell
git clone https://github.com/jianan757-gif/Prompt-Shelf.git
cd Prompt-Shelf
python -m pip install Pillow
python scripts/generate-fixtures.py
cd poc
pnpm install --frozen-lockfile
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'src-tauri\target-stage-a'
pnpm tauri build --no-bundle -- --locked
cargo build --manifest-path src-tauri/Cargo.toml --release --bin prompt-shelf-store --locked
cd ..
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\scripts\start-stage-a.ps1" -Fixture synthetic
```

样本生成器拒绝覆盖已有 `fixtures/100x500`。ExecutionPolicy Bypass 仅用于该 PowerShell 进程。构建输出在 `poc/src-tauri/target-stage-a/release/`，不生成安装器。上述脚本将合成样本显式导入 `evidence/stage-a/manual-synthetic`，重启继续读取同一 SQLite 库，不覆盖旧 PoC；若需验证空库改用 `-Fixture empty`（表示不导入样本，不会清空已有数据）。

首次启动默认隐藏：在专用目标输入框内按 Ctrl+Alt+1 呼出面板，点击提示词或图片。更换目标时先点新目标输入框，再按热键重新绑定。托盘可打开主库或退出；启动脚本拒绝同时运行同项目多版本。私人混合样本不随仓库提供，请先使用 synthetic。

## 数据目录与离线工具

直接启动新 exe 默认使用 Tauri 本地应用数据目录（应用标识 `io.github.jianan757-gif.promptshelf`），首次为空库；不会自动载入 JSON 样本或旧 PoC 数据。`--data-dir <目录>` 可指定隔离测试库。目录为 `library.db`、`assets/`、`cache/thumbs/`、`staging/`、`logs/`；完整恢复切库机制留在阶段 E。

无需启动 GUI 即可验证持久数据：

```powershell
.\poc\src-tauri\target-stage-a\release\prompt-shelf-store.exe --data-dir .\evidence\stage-a\offline-library --import-manifest .\fixtures\100x500\manifest.json --check
.\poc\src-tauri\target-stage-a\release\prompt-shelf-store.exe --data-dir .\evidence\stage-a\offline-library --check
```

导入仅允许空库。重复导入非空库会拒绝，不会合并或覆盖；离线工具与 GUI 不能同时占用同一数据目录。源图片只读复制，原图按 SHA-256 共享，缩略图独立生成并按文件授权。旧 `scripts/start-poc.ps1` 仅供本地保留的 round7 历史程序使用。
## 验证命令

供需要复现的审查者使用；本轮只运行阶段 A 相关测试和必要回归，不重复完整 PoC。

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
| `poc/src-tauri/src` | SQLite、路径与资产服务，以及既有窗口、队列、Esc 和平台适配器 |
| `scripts` | 样本生成、启动、校验及资源采样 |
| `tests/receiver` | 本地 paste 接收页，不代表真实应用验收 |
| `docs/public` | 公开技术交接报告 |
| `docs/validation` | 已有验证记录的公开副本 |
| `Prompt Shelf 4.2·v2.0` | 任务书与 HTML 原型 |

依赖、编译产物、私人截图、完整本机日志、历史源码 ZIP 和机器环境清单均不纳入版本控制。源码以 MIT 许可提供；第三方依赖适用其各自许可证。
