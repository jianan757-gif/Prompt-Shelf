# Prompt Shelf 4.3 Windows PoC

Tauri 2 + Rust + React/TypeScript 技术验证。仅实现快捷调用、受控图像访问、粘贴队列与代表性主库浏览；不是完整产品，不进入 4.4。正式存储方向仍为 SQLite + 独立原图，本轮 JSON manifest 只是可维护的样本接口。

## 构建与验证

环境：Windows、Node 24、pnpm 11、Rust MSVC、VS 2022 Build Tools、WebView2。版本由 `pnpm-lock.yaml`、`src-tauri/Cargo.lock` 固定。

```powershell
cd poc
pnpm install --frozen-lockfile
pnpm test
$env:CARGO_TARGET_DIR = Join-Path $PWD 'src-tauri/target-round6'
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --no-bundle -- --locked
```

本轮产物：`src-tauri/target-round6/release/prompt-shelf-poc.exe`。旧round1–5 exe与报告保留，不覆盖正在运行的版本。无需开发服务器。`pnpm tauri dev` 用于开发，`pnpm dev` 单独运行只能预览界面，不能验证系统功能。

## 启动与操作

以下由用户在专用测试输入框准备好后手动执行。本轮交付没有自动运行 GUI 或系统粘贴。

```powershell
$env:PROMPT_SHELF_MANIFEST = (Resolve-Path '../fixtures/100x500/manifest.json').Path
& './src-tauri/target-round6/release/prompt-shelf-poc.exe'
```

不设环境变量时，从当前目录、上一级、上两级搜索 `fixtures/100x500/manifest.json`；找不到则使用无图内置演示。显式指定的 manifest 无效会中止启动，不静默换数据。测试期间保留样本目录，HDROP 直接引用其持久原图，不生成临时原图。

- 在非敏感目标输入框内按 Ctrl+Alt+1/2/3 呼出全部/最近/收藏；显示前捕获目标，面板内切页保留目标。
- 从托盘打开面板仅显示已有明确绑定，不捕获托盘窗口、不更换目标；未绑定时先激活外部目标输入框，再用Ctrl+Alt+1/2/3绑定。搜索和点击面板不会重绑。
- Enter/点击文字粘贴全文；效果图/参考图按钮粘贴单张；44×90 顺序按钮逐项触发参考图，间隔可调。
- 快捷面板渲染最多50条匹配案例，显示总匹配数；主库浏览完整样本。
- 活动请求期间全局 Esc 中止后续发送；无请求时全局 Esc 隐藏面板并注销；队列完成仍保留可见会话的 Esc。文字、单图、多图共用一个 Rust 请求保护。
- 首次恢复被系统拒绝、目标关闭/身份变化、用户切到其他前台均停止发送。恢复只尝试一次；后续不会重新抢回。释放 Ctrl/Alt/Shift/Win 后再点击发送。
- Round5启动仍隐藏热态；唤出后置顶可正常输入，连续调用时保持可见。每请求只恢复一次绑定目标，最多等待600ms，确认后才写入/发送；粘贴完成不抢回焦点。目标/第三方/面板交互切换会停发，不自动续队列。窗口X/系统关闭先取消再收起；活动Esc只取消且保留。默认560/min540，round4圆箭头保留。用户自行先退出旧版，再启动新版，代理不关闭程序。
- 错误保留在面板底栏、托盘状态菜单及 tooltip；快捷键重新打开面板可查看。错误不会自动抢走用户焦点。
- 托盘“主库浏览”按需新建窗口；主库关闭销毁，快捷面板保留自身可见状态，只有主动收起或全局空闲Esc才隐藏。托盘“退出”取消请求后退出。
- done/total **只表示已触发粘贴**，目标接收和上传完成必须另外观察。

## 数据接口

环境变量 `PROMPT_SHELF_MANIFEST` 指向 schema v1 JSON；路径相对其所在目录：

```json
{
  "schema_version": 1,
  "fixture_kind": "synthetic",
  "assets": [{"id":"a1","original":"assets/中文路径/a.png","thumb":"cache/thumbs/a.png","sha256":"64位SHA256","width":1200,"height":900,"format":"png","bytes":12345}],
  "cases": [{"id":"c1","title":"测试案例","prompt":"完整提示词","project":"验证","tags":["精修"],"fav":true,"used":1,"result":"a1","refs":["a1"]}]
}
```

id 为字符串，result 可为 null。original/thumb 必须存在、相对、真实路径不越过 manifest 根、且所有缩略图与所有原图路径分离。仅独立缩略图的具体文件进入 asset protocol allowlist；原图仅 Rust 使用，不经 IPC/Base64，不覆盖、不清理。PNG 原字节用于 PNG format；JPG/WebP 的 PNG/DIB 输出属于兼容转换，HDROP 仍引用原文件。DIBV5 有 alpha 掩码；DIB 的传统兜底没有 alpha 保证。ICC、目标重新编码和原尺寸实际接收待测。`--verify-assets` 会在启动时校验全部提供的 sha256；自动化测试已校验两个主控样本集。

## 目录与诊断

```text
src/                  React 快捷面板、只读主库、搜索与可见缩略图
src-tauri/src/data.rs  manifest 校验及小体积前端投影
src-tauri/src/queue.rs 唯一异步队列、取消与发送临界区
src-tauri/src/platform Windows 适配器及非Windows未实现边界
reports/              阶段报告、原始构建/测试输出、产物哈希
```

日志由 Tauri `app_local_data_dir()/logs/poc.jsonl` 生成；托盘菜单和 `get_boot` 返回准确路径。包含进程号、目标 HWND/PID/线程/进程创建时间/捕获时间、队列阶段/当前资产 id/时间戳、唤醒 token、React 搜索框与 document 焦点确认后的 rAF ready、主库冷建/销毁时刻。日志不记录提示词全文、图像字节或窗口标题。`requestId` 是本次进程内唯一队列/请求 id。

`search_interactive` 从实际输入事件到结果提交后双rAF计时，包含结果数、窗口、案例/资源数；覆盖输入单列 `search_superseded`，不记录搜索词。该埋点用于后续WebView响应P95，不能用纯算法单测耗时替代。

资源测量须覆盖 Rust 与所有关联 WebView2 进程。用户已选择先测基线再定阈值；没有资源达标结论。主库 WebView 销毁不保证 WebView2 共享进程立即释放。macOS 仅保留平台和标题栏边界，系统粘贴明确未实现，未做跨平台编译/实机验证。

Round5会话：热键呼出保留搜索、项目/标签筛选、选中和列表/参考图位置；1/2/3仍切换集合，各集合保留浏览位置。粘贴事件不更新used导致重排。活动请求期间回到面板搜索/浏览会取消未发送项，待取消完成后再明确点击下一次；不会因回访自动恢复目标。准备期面板取消带requestId，native Focused仅在已恢复目标后且实际前台为quick_panel时取消，迟到初始激活通知不凭busy取消。失败状态与绑定提示在面板底栏保留。
Round6 Esc：面板唤出成功后注册，队列结束不注销；活动时一次 Esc 仅取消，释放后再次 Esc 才收起并注销。隐藏状态不持有全局 Esc。注册失败在底栏/托盘明确显示并拒绝粘贴；可用面板本地 Esc 或 X 收起后重新唤出重试。注销失败保留可见及所有权，不虚报隐藏；隐藏失败会明确显示已释放 Esc。全局 Pressed/Released 与本地 keydown/keyup 由后端统一裁决，长按与本地重复不触发第二次动作。
