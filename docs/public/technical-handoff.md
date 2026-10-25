# Prompt Shelf 4.3 技术验收与交接报告

日期：2026-10-25（Asia/Shanghai）｜交接版本：4.3-round7｜用途：供 ChatGPT 审查并制定 4.4 方案。

**结论：用户已确认“测试通过，Prompt Shelf 4.3 Windows 核心技术 PoC 已完成验证”，本次按此结论完成 4.3 成果交接。** 当前代码是可运行的 Windows PoC，不是正式产品。本次仅阅读、整理和归档已有成果，没有新增功能、重构或重跑测试，也没有进入 4.4。

验收结论与证据完整度分别记录：最新用户反馈确认上一轮光标修复测试通过，并确认阶段完成；它没有提供逐应用兼容性清单或性能数值。因此以下缺少专项证据的内容仍标记“未验证／未形成可核验记录”，不推定全部矩阵达标。

文中路径除特别说明外均相对项目根目录。此为公开审查版：历史本机证据路径用于说明来源，不保证公开仓库中存在；已公开证据集中在 docs/validation，未公开私人截图、完整运行日志或机器环境清单。历史报告记录当时状态，最终交接状态以本报告为准，旧报告不回写为全部通过。

## 1. PoC 验收结果

| 验收项 | 实际结果与边界 | 测试方式 | 已有证据位置 |
|---|---|---|---|
| 全局快捷键与窗口呼出 | 用户确认托盘、快捷键、搜索自动获焦通过；实现 Ctrl+Alt+1/2/3 切换集合。三个快捷键逐一覆盖情况没有单独清单 | Windows 用户实测；代码检查；唤醒日志 | `docs/manual-round1.md`、`docs/manual-round2.md`、`evidence/manual-round2/`、`evidence/handoff/poc-through-20261025.jsonl` |
| 目标窗口记录及焦点恢复 | 回到原记事本并粘贴通过；连续调用时面板保持显示，搜索状态保留，用户确认 round5 前四项实现。拒绝恢复、目标变化等异常路径有自动测试，不能视为全部异常场景实测通过 | 用户实测；Rust 模拟测试、身份/前台诊断日志 | `docs/round5-report.md`、`docs/verification-round5.md`、`evidence/independent-round5/` |
| 文本、单图跨程序粘贴 | 完整文本及换行已有用户明确通过记录；单图调用有运行记录，用户总体确认基本功能完成，但没有逐目标单图接收、原始尺寸/透明度/颜色的完整记录。round7 文字粘贴后光标残留修复已由本轮用户确认通过 | 用户实测；格式准备单测；运行日志（仅证明内部流程） | `docs/manual-round2.md`、`poc/reports/round7/`、`evidence/handoff/user-acceptance.md`、日志中 `request_reserved`/`queue` |
| 多图顺序、进度反馈、Esc 中止 | 队列及常驻连续调用已实现，用户反馈曾完成队列粘贴；Esc 忙时停止后续派发，空闲时直接隐藏，隐藏后注销全局 Esc。去重/取消有自动测试；最新总体通过未附逐张接收及长按 Esc 专项清单。进度是“已触发”，不是目标上传完成 | 用户反馈；Rust 队列模拟和 Esc 状态单测；前端请求隔离测试 | `docs/round6-report.md`、`evidence/controller-round6/results.json`、`poc/src-tauri/src/queue.rs`、`escape.rs`、归档运行日志 |
| ChatGPT、豆包等真实应用兼容性 | 用户指定豆包客户端为常用目标；尚无按 ChatGPT Web、豆包 Web、豆包客户端分别列出的接收结果/版本/截图，专项兼容性无法从现有证据确认。日志不记录应用名称，不能按 PID 猜测应用 | 总体用户验收；专项矩阵未形成可核验记录 | `docs/windows-manual-test.md` 是测试协议，不是通过记录；`evidence/handoff/user-acceptance.md` |
| 双窗口生命周期及资源管理 | 快捷面板隐藏保热、主库按需创建/复用/关闭销毁已实现；缩略图按可见性加载。未发现主库创建/销毁专项实测记录或 WebView2 释放数据，不能认定资源回收达标 | 代码审查；实机资源专项未验证 | `poc/src-tauri/src/main.rs`、`poc/src/App.tsx`、`docs/verification-code.md`、`poc/reports/S3.md` |
| 100 案例／500 图片性能 | 两套样本完整性检查通过，启动日志确认 100/500 实际加载；有零散唤醒、搜索耗时事件，但没有完整同版本基线/P50/P95或资源采样报告。不能判定性能达标；用户此前要求先测基线再定阈值，阈值尚未确认 | 样本自动校验、真实运行埋点；完整性能验收未形成记录 | `evidence/100x500-integrity.json`、`100x500-mixed-integrity.json`、`evidence/handoff/poc-through-20261025.jsonl` |

### 已有自动验证与性能资料如何解读

- round5：独立复核 Rust **27/27**、前端 **15/15**、TypeScript/Vite 通过，记录于 `evidence/independent-round5/`。该报告中的空闲 Esc 规则是历史行为，已被 round6 替代。
- round6：主控补测 Rust **32/32**、前端 **15/15**及构建通过，36 个冻结源码文件匹配。独立验证因用量限制中断，仅有局部静态审查，不能称为独立全项通过。见 `evidence/controller-round6/`、`docs/verification-round6.md`。
- round7：前端 **15/15**、TypeScript/Vite、Windows release 构建及启动脚本语法检查通过；Rust 仅变更版本标识，未重复全套 Rust 测试。见 `poc/reports/round7/build.json`、`frontend-tests.txt`、`build.txt`。构建时的 pending 人工状态由本次用户反馈补充，保留原构建记录不覆盖。
- Rust 队列测试主要使用 Mock 平台；剪贴板格式测试检查数据布局，不证明目标软件实际接收。前端单测覆盖搜索、图片轨道计算、会话/消息隔离，不是浏览器端视觉测试。
- 交接日志快照包含多个历史运行版本，共 65 个 `quick_interactive`、6 个 `search_interactive` 事件；它们不是按统一条件采集的性能样本，未混算为性能结论。快照内没有主库生命周期事件。计数索引见 `evidence/handoff/log-inventory.json`。
- `evidence/tooling-review/sampler-selftest-*.csv`/`*.json` 是**采样工具自测**，不是 PoC CPU/内存基线。`scripts/sample-process-tree.ps1` 是已有采样器；没有发现完整 PoC 资源 CSV。本次没有补跑。

## 2. 技术实现与代码交接

### 技术栈与依赖

Windows 桌面壳采用 **Tauri 2 + Rust 2021 edition + React 19 / TypeScript 5.9 + Vite 7**，WebView2 渲染。依赖声明见 `poc/package.json`、`poc/src-tauri/Cargo.toml`，精确解析版本分别以 `pnpm-lock.yaml`、`Cargo.lock` 为准。

关键依赖：`tauri-plugin-global-shortcut`（系统热键）、`windows-sys 0.61`（直接调用 Win32）、Tokio（异步任务与等待）、Serde/serde_json（IPC/manifest/日志）、image 0.25（PNG/JPEG/WebP 解码及兼容表示）、sha2（校验）、dunce（Windows 路径规范化）。Python/Pillow 只用于样本工具，不是应用运行依赖。当前未接入 SQLite，也未生成安装器。

### 关键模块

| 文件/模块 | 职责与交接要点 |
|---|---|
| `poc/src-tauri/src/main.rs` | 托盘、热键、目标绑定、Tauri commands/events、双窗口生命周期与 JSONL 埋点；主库异步创建，避免在同步回调中建 WebView；执行粘贴时保持面板可见、焦点交给目标 |
| `poc/src-tauri/src/platform/mod.rs`、`platform/windows.rs` | `PastePlatform` 隔离目标捕获、焦点恢复、格式准备、写剪贴板和派发；Windows 具体接口集中在适配器 |
| `poc/src-tauri/src/queue.rs` | 单活动请求、顺序执行、后台准备图片、可调间隔、取消及焦点检查、按请求 ID 反馈状态；取消不能撤回已派发项 |
| `poc/src-tauri/src/escape.rs` | 面板可见期间全局 Esc 会话；忙时取消，空闲隐藏；按下/释放去重，隐藏注销 |
| `poc/src-tauri/src/data.rs` | schema v1 JSON manifest 加载，案例/资源引用、路径边界、原图与缩略图分离及可选哈希核验；目前是只读样本数据层 |
| `poc/src/App.tsx`、`session.ts` | 两窗口 React 视图、invoke/listen、请求 ID 隔离、迟到消息处理、搜索/筛选/位置保留；窗口可见性和 IntersectionObserver 控制图片加载 |
| `poc/src/search.ts`、`ReferenceCarousel.tsx`、`carousel.ts` | 多词 AND、标题权重、词典/安全高亮；逐张参考图移动、位置保存及边界箭头 |
| `poc/src/poc.css`、`quick.css`、`library.css`、`carousel.css` | 当前样式组合；560 默认宽度、V4.11 圆形半透明图片箭头、无原生图片滚动条。round7 在状态区显示转圈，移除文字按钮系统等待光标 |
| `poc/src-tauri/tauri.conf.json`、`capabilities/default.json` | 窗口/CSP/权限边界；asset 协议仅授权加载的缩略图文件。`scripts/start-poc.ps1` 固定样本、检测旧进程并记录启动哈希 |

### Windows 原生链路与通信

1. 外部热键唤醒前调用 `GetForegroundWindow` 捕获目标，用 HWND、`GetWindowThreadProcessId` 得到的 PID/线程 ID、`GetProcessTimes` 的进程创建时间标识并校验目标。托盘只浏览已有绑定，不能用托盘窗口替换目标；队列忙时不重绑。
2. 粘贴开始后调用一次 `SetForegroundWindow`，等待并核实实际前台；恢复被拒或目标身份变化则停止。准备、写剪贴板、派发及等待边界继续检查目标，用户切窗/回点面板时停止后续任务，避免继续投递。
3. 文本使用 CF_UNICODETEXT（UTF-16）；图片提供注册的 PNG 格式、CF_DIBV5、CF_DIB、CF_HDROP。通过 `GlobalAlloc` 等分配内存，再 `OpenClipboard`、`EmptyClipboard`、`SetClipboardData`、`CloseClipboard`；以 `SendInput` 派发 Ctrl+V。
4. PNG 表示保留源 PNG 字节，CF_HDROP 指向磁盘原图；JPEG/WebP 的兼容 PNG/DIB 经过解码转换。目标自行选择格式，不能把“原图文件未修改”理解为所有目标都收到原文件字节。CF_DIB 不保留 alpha，ICC/颜色一致性未完整验证。
5. React 通过 `@tauri-apps/api/core` 的 `invoke` 调用 Rust commands，通过 `listen` 接收 `wake`、`panel-visibility`、`escape-status`、`queue-progress`。事件用 requestId 隔离；前端仅传资源 ID/索引，不把原图 Base64 经 IPC 往返。缩略图走 `convertFileSrc`。

### macOS 预留与复用边界

已有 `PastePlatform` trait、条件编译平台选择和 UI 的 macOS 标题栏布局分支。所有非 Windows 系统目前进入 `platform/unsupported.rs` 并返回 `non_windows_not_implemented`；**没有实现或验收 macOS 焦点、剪贴板、快捷键权限和系统输入链路**。

可作为正式开发基础复用：Windows 适配器、队列及取消边界、Esc 会话、请求隔离、双窗口组织、搜索/轮播逻辑和相应测试，以及已确认的 UI 交互。复用现有保护机制即可，不需要为交接额外叠加防御层。

正式开发需要调整：将只读 manifest 和合成数据换成已定技术方向的 SQLite 元数据与本地原图/缩略图存储；接入真实新增/编辑/持久化、最近使用和收藏等业务；把当前只读/PoC 占位入口接入正式模块。快捷面板前 50 条显示限制、内存搜索、图片生命周期策略应结合正式数据规模决定是否调整，不因本次交接先行重构。

## 3. 遗留事项与 4.4 建议

1. **证据缺口**：逐真实应用的图文接收、原图尺寸/透明度/色彩、主库关闭后的进程资源释放、完整 100/500 性能基线及阈值仍缺少可核验专项记录。用户已确认阶段完成，后续审查应保留这些限制，不能补写为通过。
2. **样本限制**：授权图片目录的 20 张 PNG 为截图，最大 2347×1389、无 ICC；混合样本其余为合成图。真实高分辨率实拍与 ICC 色彩验收没有补齐；合成嵌入 ICC 样本不能替代该验收。
3. **系统边界**：前台核验与 SendInput 不是原子事务；Windows 可能拒绝激活或输入，目标内部编辑焦点/上传速度由应用决定。当前按间隔派发而非等待上传确认；Esc 无法撤回已派发内容。面板可见时全局 Esc 会接管目标软件的 Esc，这是当前已实现交互，隐藏后释放。
4. **产品化尚未做**：素材导入与编辑、标签/项目管理、持久收藏及最近使用、回收站、备份恢复、数据库迁移、安装/升级分发均不是本轮完整产品交付。当前前端 package 为 0.4.3，Cargo/Tauri 为 0.1.0，正式发布时需要统一版本管理；本次保留现状。

4.4 建议先承接已验证的系统粘贴、队列、窗口与 UI 基线，再补上述正式数据与业务模块；由 ChatGPT 在审查本报告后制定具体范围和顺序。无需重新实现已通过的链路。本次不启动 4.4。

## 4. 公开交付信息

- 仓库：https://github.com/jianan757-gif/Prompt-Shelf；分支 main；版本 v0.1.0。准确 Commit ID 以 GitHub 标签为准。
- 原交接时没有 Git 仓库，本次首次建立公开源码仓库；不是历史提交重建。
- 源码沿用 round7，仅将前端 package 版本从 0.4.3 对齐为 0.1.0，与 Cargo/Tauri 一致，未修改功能。历史测试针对相应快照，本次不重跑或伪造结果。
- 原 Windows round7 可执行文件 SHA-256：772e50e834ce416888fa732c816ee71c72ab46eee37ca9665e20c26a1234c174。该程序属于此前构建，本次发布仅源码，不冒充本提交重新构建的二进制。
- docs/validation 保存公开测试日志及用户验收摘录，路径已脱敏；原始运行日志与私人图片只保留本机，未上传。
- 完整性能基线及逐应用接收矩阵仍未形成记录，不因公开发布而改变验收证据边界。
- 本轮停留 4.3，4.4 尚未启动。
