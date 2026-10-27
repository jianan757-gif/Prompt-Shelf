# Prompt Shelf

Windows 优先的图文提示词管理与快捷调用桌面工具，采用 **Tauri 2 + Rust + React/TypeScript**。

**main 已合并经用户与独立审核通过的 B1，回退标签为 `baseline-b1`。** 当前开发分支交付 B2 r2，已于 2026-10-27 获用户全部测试通过确认；B2 结项，不代表已发布软件。见 [B1 业务交付记录](docs/public/b1-business-delivery.md)及 [B2 交付记录](docs/public/b2-edit-safety-delivery.md)。v0.1.0 历史发布不变；macOS 仅预留适配接口。

## 审查入口

- [4.4 架构与实施计划](Prompt%20Shelf%204.4%20正式开发架构与实施计划.md)、[阶段 A 交付说明](docs/public/stage-a-delivery.md)。
- [B1 主库 UI 交付与视觉验收状态](docs/public/b1-ui-delivery.md)、[公开验证摘要](docs/validation/b1-ui-results.json)。阶段 A 回退点：独立提交 `b349f61`，标签 `baseline-stage-a`；B1 UI 与业务为其后独立提交，main 以 `ce43010` 保留历史合并 B1。
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

阶段 A 已接入 SQLite 数据读取与显式离线样本导入。本地 B1 已实现新建、编辑、手动保存及图片基础管理；本工作树新增自动草稿、恢复及未保存关闭保护；回收站、备份和安装升级仍留在后续阶段。目标软件可能选择不同剪贴板格式或转码；进度代表已派发，不代表上传完成。完整逐应用兼容性矩阵、资源基线及性能阈值仍缺专项记录。

## 本地构建与启动（Windows）

需要 Windows、WebView2、Rust MSVC、Visual Studio C++ Build Tools、Node.js（支持 `--experimental-strip-types`）、pnpm；生成样本另需 Python 与 Pillow。依赖版本以两个 lockfile 为准。

```powershell
git clone --branch main https://github.com/tangchiji/Prompt-Shelf.git
cd Prompt-Shelf
cd poc
pnpm install --frozen-lockfile
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'src-tauri\target-stage-b1-business'
pnpm tauri build --no-bundle -- --locked
cargo build --manifest-path src-tauri/Cargo.toml --release --bin prompt-shelf-store --locked
cd ..
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\scripts\start-b1-business.ps1" -Fixture empty
```

样本生成器拒绝覆盖已有 `fixtures/100x500`。ExecutionPolicy Bypass 仅用于该 PowerShell 进程。当前构建输出在 `poc/src-tauri/target-stage-b1-business/release/`，不生成安装器。上述脚本使用 `evidence/b1-business/manual-empty`，首次为空库，重启继续读取同一 SQLite 库；`empty` 表示不导入样本，不会清空已有数据。需要合成样本时，先安装 Pillow 并运行 `python scripts/generate-fixtures.py`，再用 `-Fixture synthetic` 启动。旧阶段 A 程序与 `start-stage-a.ps1` 保留。

首次启动默认隐藏：在专用目标输入框内按 Ctrl+Alt+1 呼出面板，点击提示词或图片。更换目标时先点新目标输入框，再按热键重新绑定。托盘可打开主库或退出；启动脚本拒绝同时运行同项目多版本。私人混合样本不随仓库提供，请使用 empty 自建测试案例或 synthetic 合成样本。

### B2 r2 开发分支构建

B2 位于 `codex/4.4-b2-edit-safety`。上述 main 构建仍为 B1；审查 B2 时，先获取远程分支并切换到该分支，再将构建输出改为 `src-tauri\target-stage-b2-r2`，分别构建 GUI 和 CLI，然后运行 `scripts/start-b2-r2.ps1 -Fixture empty`。默认使用独立的 `evidence/b2-r2/manual-empty`，不会清空或复用 B1 测试库。

r2 修复了首轮自动草稿的浏览器定时器错误，并接入用户确认的图片胶囊与拖动排序，详见 [r2 验证和测试清单](docs/validation/b2-r2/README.md)。旧程序及旧测试库保留。

B2 数据库迁移至 schema 2；B1 程序会拒绝读取，代码回退不等于数据降级。实测前使用独立库，勿用个人资料直接试验。

## 数据目录与离线工具

直接启动新 exe 默认使用 Tauri 本地应用数据目录（应用标识 `io.github.jianan757-gif.promptshelf`），首次为空库；不会自动载入 JSON 样本或旧 PoC 数据。`--data-dir <目录>` 可指定隔离测试库。目录为 `library.db`、`assets/`、`cache/thumbs/`、`staging/`、`logs/`；完整恢复切库机制留在阶段 E。

无需启动 GUI 即可验证持久数据：

```powershell
.\poc\src-tauri\target-stage-b1-business\release\prompt-shelf-store.exe --data-dir .\evidence\b1-business\offline-library --import-manifest .\fixtures\100x500\manifest.json --check
.\poc\src-tauri\target-stage-b1-business\release\prompt-shelf-store.exe --data-dir .\evidence\b1-business\offline-library --check
```

导入仅允许空库。重复导入非空库会拒绝，不会合并或覆盖；离线工具与 GUI 不能同时占用同一数据目录。源图片只读复制，原图按 SHA-256 共享，缩略图独立生成并按文件授权。旧 `scripts/start-poc.ps1` 仅供本地保留的 round7 历史程序使用。
## 验证命令

供需要复现的审查者使用；B2 使用定向草稿与关闭测试，不重复完整 PoC。

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
