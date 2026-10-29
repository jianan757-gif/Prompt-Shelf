# Prompt Shelf 0.2.0-beta.1 · Windows 候选版

本地提示词与图片资料库，使用 Tauri 2 + Rust + React/TypeScript。已实现案例编辑、自动草稿、分类检索、收藏与最近使用、回收站、ZIP 整库备份恢复，以及 Windows 托盘与快捷调用。本版为 F 交付候选版，不是稳定版；保留已验收 A–E 行为。

## 构建与验证

环境：Windows、Node 24、pnpm 11、Rust MSVC（当前 1.98.1）、VS 2022 Build Tools、WebView2。锁文件为 pnpm-lock.yaml 和 src-tauri/Cargo.lock。

```powershell
cd poc
pnpm install --frozen-lockfile
pnpm test
pnpm build
$env:CARGO_TARGET_DIR = Join-Path $PWD 'src-tauri/target-stage-f'
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --bundles nsis -- --locked
cargo build --manifest-path src-tauri/Cargo.toml --release --bin prompt-shelf-store --locked
```

F 的两个 exe 位于 src-tauri/target-stage-f/release/：prompt-shelf-poc.exe（主程序，内部文件名保留）、prompt-shelf-store.exe（离线工具）。安装包在 release/bundle/nsis/。仓库根 scripts/build-stage-f.ps1 负责正式候选构建，scripts/start-stage-f.ps1 负责启动。自测报告见 reports/stage-f/；安装与升级验收及资源基线由主控单独记录，编译/Mock 验证不代替 Windows UI 手测。

## 数据目录与启动

生产 application identifier 为 io.github.jianan757-gif.promptshelf，与旧 local.promptshelf.poc 隔离。默认使用 %LOCALAPPDATA%/io.github.jianan757-gif.promptshelf，首次为空库。不扫描 manifest、演示案例或 PROMPT_SHELF_MANIFEST，不自动迁移旧 PoC 数据。

GUI 仅支持可选 --data-dir <directory>，测试/开发指定独立目录：

```powershell
& './src-tauri/target-stage-f/release/prompt-shelf-poc.exe' --data-dir 'D:/PromptShelf-test/library-a'
```

主程序先初始化官方 single-instance 插件，再运行其他插件和 setup。重复启动不执行导入、数据打开、热键注册或目标改绑。单库单连接受 Mutex<Connection> 保护，SQL/文件工作在 blocking worker 中进行。Store 全生命周期持有 .library.lock 的 OS 锁，离线工具与 GUI、两个离线工具不能同时打开同一目录；进程退出由 OS 释放，不删除锁文件。

首次兼容已有平面资料库，恢复后 active.json 指向同根 storage/<uuid>/；不更改 identifier，不自动迁移其他旧标识的数据。库根不可放在安装目录内。

```text
<data-dir>/
  active.json, active.previous.json  E 起使用的当前/上一库指针（有则读取）
  storage/<uuid>/                   整体恢复产生的资料库
  library.db, assets/               平面旧库兼容；schema 2
  cache/, staging/, logs/           缓存、临时文件及运行日志
  .library-host.lock               应用根锁
  .library.lock                    各资料库锁
```

安装使用官方 NSIS 当前用户模式，显示名称 Prompt Shelf，候选版本 0.2.0-beta.1。已装 WebView2 可直接使用；缺失时由官方 bootstrapper 下载，需网络。安装后可从开始菜单或安装结束页启动，应用驻留托盘；Ctrl+Alt+1/2/3 打开快捷面板，托盘“主库浏览”打开主库，窗口关闭遵循已有未保存处理。没有新增登录自启。

升级/覆盖安装前退出应用，使用相同 identifier 和数据根。正常卸载只删除程序与快捷方式；最小 NSIS hook 强制保留资料库。官方卸载页的“删除应用数据”框仍存在，若勾选会提示资料仍保留，不能据此清理资料。重新安装后沿用原数据；主动清理请在退出后自行处理明确的资料目录。安装器行为须用独立测试 identifier 和私有安装目录验证，不能拿真实用户库做安装测试。

## 显式开发样本导入与离线检查

prompt-shelf-store 是独立 console binary，无 Tauri Builder、窗口、托盘、热键或系统输入。必须提供 --data-dir，可附 --import-manifest <path> 和 --check。每次成功均执行完整性/外键/原图哈希检查，输出一行 JSON；失败非零退出。

```powershell
& './src-tauri/target-stage-f/release/prompt-shelf-store.exe' --data-dir 'D:/PromptShelf-test/library-a' --import-manifest '../fixtures/100x500/manifest.json' --check
# 导入进程退出后，以另一进程重开
& './src-tauri/target-stage-f/release/prompt-shelf-store.exe' --data-dir 'D:/PromptShelf-test/library-a' --check
```

先关闭持有同目录的程序；CLI 仍检测占用并拒绝打开。导入只允许空库，使用原 schema-v1 fixture 格式；original/thumb 必须是 manifest 根内存在的相对文件且互不混用。旧 thumb 只作兼容输入验证，正式缩略图从新保存原图重建。旧 ID 映射为新 UUID；同 hash 一资产/一原图，同案多位置、效果图/参考图与跨案共享保留为独立引用行。

流式复制/哈希 → 同卷 staging 校验格式/解码/哈希并同步文件 → 不覆盖落入 assets → 独立缩略图 → 一个事务提交全部资产/案例/分类/图片引用。失败不留下半写案例；中断最多留下未引用的完整原图/缓存，A 不自动物理清理。源文件不重编码、移动或覆盖。增量 Store::import_image 供 B 接入，仅登记图片，不创建案例或草稿。

## 资产与显示边界

PNG/JPEG/WebP；单源文件上限 512 MiB，解码宽高各上限 32768、分配预算 512 MiB。缓存高约180px、小图不放大、按比例 PNG RGBA；应用 EXIF 方向并传递 ICC profile。合成 alpha/方向/ICC 元数据测试不代表真实相机和显示器色彩验收。

缺失/损坏缓存可重建；重建前校验原图 hash，不从损坏原图生成该 hash 的预览。有效缓存命中不全量哈希原图。get_library 与启动共用 authorize_thumbnails，只对已验证缓存文件逐个 allow_file；不开放原图目录、邻文件或整盘。原图仅由 Rust 粘贴链路读取。

## 使用与本阶段限制

在目标输入框按 Ctrl+Alt+1/2/3 呼出；Enter/文字、单图、参考图队列仍使用原请求保护。面板粘贴后可见置顶、目标持焦，搜索/筛选和轨道位置保留。活动 Esc 仅取消，释放后闲时 Esc 收起并注销；隐藏后不持有全局 Esc。done/total 只表示已触发，不代表目标收到或上传完成。

自动化覆盖临时资料库、生产 Rust 逻辑与实际 React 两窗，未发送系统粘贴。Windows 安装/升级/卸载/重装、真实目标接收、托盘/重复启动/DPI 和高分辨率/真实 ICC 素材由主控及用户验收；macOS 未验证。资源指标目前只采基线，阈值待用户确定。

## 目录与依赖

```text
src/                            现有 React UI、搜索、图片轨道
src-tauri/src/paths.rs           活跃资料库内路径
src-tauri/src/storage_host.rs    根锁、active 指针与整库切换
src-tauri/installer/hooks.nsh    卸载保留资料
src-tauri/src/store.rs           SQLite、导入、DTO 查询
src-tauri/src/migrations/        事务迁移
src-tauri/src/assets.rs          原图与可重建缩略图
src-tauri/src/scope.rs           缓存文件授权
src-tauri/src/bin/               离线 CLI
src-tauri/src/platform/          保留的系统粘贴适配
src-tauri/src/queue.rs,escape.rs  保留的队列和 Esc
src-tauri/tests/                 CLI 真实进程、scope MockRuntime
reports/stage-f/                 F 自测证据
```

新增依赖：rusqlite 0.41.0（bundled/backup，MIT）、libsqlite3-sys 0.39.1 自带 SQLite 3.54.1（public domain）；官方 tauri-plugin-single-instance 2.5.4（MIT/Apache-2.0）、tempfile 3.27.0（MIT/Apache-2.0），UUID 复用锁定的 1.x（MIT/Apache-2.0）。已有 image、sha2、Serde 等继续使用，实际版本见锁文件和 CLI sqliteVersion。官方依据：[单实例](https://v2.tauri.app/plugin/single-instance/)、[SQLite 版本](https://www.sqlite.org/changes.html)、[rusqlite](https://docs.rs/rusqlite/)。

以下 B–E 分节保留各阶段能力与当时验证边界；其中旧构建路径和停止范围只描述对应阶段，当前交付以本文开头 F 说明为准。

## B1 业务范围与使用

新建案例或点击铅笔，在原卡片内编辑标题、项目自由输入、逗号标签、完整正文和来源。项目支持既有名称提示；保存时按 name_key 复用或登记分类，取消不登记。来源失焦/保存规范化域名为 HTTPS，Rust 只接受 HTTP/S；来源图标经受控 Windows Shell 打开默认浏览器。

图片可用文件选择、系统文件拖放或 Ctrl+V 导入。选中效果图/参考图后精准替换，未选中默认追加；单槽替换仅一张，追加可多张。拖动参考图调整顺序，移除仅删编辑态引用。导入或重排后恢复默认追加。文本剪贴板正常交给输入框，图片读取/解码在 Rust；无活动编辑不新增案例。原始 PNG/文件字节复用不可变资产存储，DIB 只有位图表示时编码 PNG。

点击保存才一次 SQLite 事务提交字段、分类和图片关系，base_revision 冲突或失败保留输入。新建未保存不进快捷面板，已有案例编辑时快捷面板读取旧正式版本。取消不写回旧数组，不物理删图片；取消/失败的未引用完整资产保守留存，无 GC。保存不改 use_count、last_used_at 或收藏。

保存后 library-changed 通知两窗口重读；监听先于首次读取，序号拒绝迟到响应，重新聚焦也从 SQLite 补拉。快捷面板按稳定案例 ID 保留选中/浏览位置；调用携带显示的 editRevision，保存重排后旧版本请求会被拒绝并刷新，避免旧下标指向新图片。queue/escape/平台输出链路保留。

B1 阶段只显示“未正式保存”，没有自动草稿承诺。编辑期间切换会丢失卡片的搜索/筛选或另开编辑，会提示先保存/取消；完整窗口关闭、托盘退出和异常中断保护在 B2，关闭仍会丢失未保存输入。不要把本阶段当作完整编辑安全交付。

B1 原产物目录为 src-tauri/target-stage-b1-business，GUI/CLI 参数沿用 --data-dir、--import-manifest、--check。复现：pnpm test；pnpm build；node scripts/check-ui-build.mjs b1-business；报告在 reports/b1-business/。新增官方 dialog 2.8.3（MIT/Apache-2.0），用于 Rust 原生文件选择；image 增启 BMP，仅用于剪贴板 DIB 转换，没有新 UI 框架。

2026-10-27 用户确认独立测试库 B1 业务清单 1～8 全部通过，并完成搜索框样式复测。自动化、构建与用户实测分别见 [公开证据](../docs/validation/b1-business/README.md)。该记录是 B1 验收结论；B2 验证单独记录。

## B2 自动草稿与编辑安全

文字修改采用 600ms 防抖、持续输入约 2 秒一次；图片添加/替换/排序/移除立即排入 SQLite 草稿写入。界面仅在当前序号获得后端确认后显示琥珀色“未正式保存 · 自动保护草稿”；写入中和失败单独标明。尚未落盘的最后输入不能承诺在断电或强杀后恢复。

主库重开会展示未完成草稿摘要（标题、正文片段、图片数和时间）；恢复只进入编辑，已有案例仍使用原基准修订，新建稿不进入正式案例或快捷面板。版本冲突拒绝覆盖并保留草稿；原目标缺失/在回收站时不会自动复活，可明确选择作为新草稿。支持稍后处理及逐份恢复。

搜索、筛选、另案编辑、新建、取消、主库本地 Esc、自定义 X、native CloseRequested/Alt+F4、window_action(close)、托盘退出都经过同一编辑处理：保存 / 继续编辑 / 放弃。保存或放弃成功才继续原动作，失败留在编辑。关闭需要后端 token 握手；没有确认不销毁，关闭请求未解除时不能新开编辑。快捷面板的队列与全局 Esc 语义保留。

手动保存将正式字段/分类/图片关系与消费草稿置于同一事务；失败保留旧正式记录及草稿。放弃仅删除草稿关系，不物理删除原图。防抖定时器会停止；活动会话、递增 save_seq 和只更新仍存在的草稿共同拒绝迟到写入。未接纳的迟到图片不回填。

schema 2 为 drafts 增加 pending_project_name、dirty，增加 draft_pending_tags 表。已登记分类仍用外键 ID；待登记项目/标签只留草稿输入，正式保存才解析或创建。被删除的已知分类通过外键归一，恢复不从旧显示名重建。迁移 1→2 在事务内执行；B1 exe 正确拒绝打开 schema 2，切回 B1 源码不会降级数据。没有降级、GC 或备份系统。

报告在 reports/b2/；实际 release CSS 检查使用 node scripts/check-ui-build.mjs b2。此为首轮 B2 记录；最终 r2 已获用户验收，自动化进程测试只打开临时目录，不迁移用户 B1 测试库。

## B2 R2：真实时钟与编辑图片胶囊

revision `4.4-b2-r2`。修复 DraftClock 原生方法被错误绑定到实例的 Illegal invocation；默认计时器通过 globalThis 显式调用，保留 600ms 防抖/连续约 2 秒和图片立即暂存。保护状态仍取决于当前序号的 SQLite 确认。此前用户普通卡片状态与该独立时钟缺陷的关系未经确认，不把自动测试结论当作用户那次无恢复事件的根因证明。

CaseEditor 删除图片框外粘贴提示栏；轨道内保留添加入口与精确选择机制。图片仍高 90px、原有宽度不变，右下两图标半透明胶囊常驻（unlink 替换、trash-2 移除），单次编号、细蓝边/圆形对勾选中。参考图拖动显示半透明跟随，其他图 FLIP 让位；本地预览不写草稿，松手才按稳定引用 ID 提交一次。Esc、pointercancel、lostcapture 或 blur 取消，其他 pointerId 不能结束本次拖动；效果图不参与。编辑态箭头点击区缩至中部，不挡底部胶囊；普通卡片和快捷面板不变。

报告 `reports/b2-r2/`；用户于 2026-10-27 确认 r2 测试清单全部通过，见 [公开人工验收记录](../docs/validation/b2-r2/user-acceptance.json)。真实浏览器测试使用实际 CaseEditor/useCaseEditor，并以测试 IPC 接到生产 Store/EditSessions/SQLite；不触碰用户库，原生剪贴板/文件拖入只模拟传输边界。测试程序只强杀自身创建的 Rust worker。

浏览器验证需要现成 Playwright 与 Edge，不是新正式依赖。先用独立 CARGO_TARGET_DIR 编译测试桥，再运行测试；如环境无法按包名解析，通过 PLAYWRIGHT_MODULE 指定现有安装路径：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'src-tauri/target-stage-b2-r2'
cargo test --manifest-path src-tauri/Cargo.toml --locked --test browser_store --no-run --message-format=json *> reports/b2-r2/browser-bridge-build.jsonl
pnpm test:browser
pnpm test:browser:controls
node scripts/check-ui-build.mjs b2-r2
```

Vite 测试服务器只监听本机并排除 Rust target/报告目录的扫描；资产请求限定在自建临时目录。所有浏览器运行按时间留独立 JSON，失败记录与成功记录分开。

## C 分类、检索与调用

revision `4.4-c`，schema 保持 2，无新增依赖。项目/标签新增、重命名、单项/批量删除以稳定 ID 操作；删分类解除正式案例/草稿关系，保留案例与图片。重命名同时绑定旧名/新名 pending 输入，标签按同一 ID 去重并保留最早位置；删除也清理等价 pending 输入。分类写入先处理当前案例编辑，再由后端持会话锁检查空闲，防止迟到字符串草稿复活旧分类。

快捷标签开关及顺序存 SQLite，主库左侧、顶部及快捷面板同源；左侧握柄可指针拖动或 Alt+上下键。收藏即时小事务，可在另一窗编辑期间更新，正式内容保存不会覆盖收藏。最近使用只在同一次队列请求首次成功派发一项后记一次；零派发不记，部分派发后取消仍记。记录失败可见且不重放已发出的粘贴。该计数仍表示派发过，不代表目标收件成功。

词典可编辑并持久化，去空/组内去重、拒绝跨组冲突；保存失败保留输入。布局、排序、二级栏列数/显示、栏宽及快捷间隔保存到白名单偏好。分类筛选用 ID，名称更新不丢选择；删除所选分类才解除筛选。两窗通过 library-changed 更新。快捷面板自动刷新保留当前会话顺序、选中 ID 和位置，切集合或再次点击当前集合应用新排序；跨集合保留目标集合已存的选中与滚动。指针选卡不强制滚动，键盘选择仍跟随。

get_library_index 只读轻量元数据与检索字段，不生成缩略图；主库初始渲染 60 条，滚动增量加载，快捷面板前 50 条。get_case_assets 按实际案例 ID 批取，每次最多 120 个；动态授权仍只覆盖对应缩略图。旧 get_library/CLI 检查继续可用。

报告 `reports/stage-c/`。必要验证包括真实 SQLite、生产队列配 MockPlatform、自建临时库的实际 React 两窗（测试 IPC transport），用户已于 2026-10-29 反馈测试全部通过，按整体用户验收记录，未附逐应用原始日志；自动化不冒充 Windows 实机测试。不做 D–F。最终 release 由主控通过 Tauri CLI 在 target-stage-c 构建，不能以普通 cargo GUI release 代替。

浏览器测试沿用现成 Playwright/Edge，不新增正式依赖：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'src-tauri/target-stage-c'
cargo test --manifest-path src-tauri/Cargo.toml --locked --test browser_store --no-run --message-format=json *> reports/stage-c/browser-bridge-build.jsonl
# 可用 PLAYWRIGHT_MODULE 指定现有包位置
pnpm test:catalog
node scripts/check-ui-build.mjs stage-c
```

## D 回收站与安全清理

revision `4.4-d`，schema 仍为 2，无迁移或新增依赖。普通卡片可移入回收站，正常列表和快捷面板立即排除；4.5 秒撤销调用同一 SQLite 恢复操作。回收卡沿用 V5.0 冻结样式，显示前三张缩略图、剩余数、删除前项目、参考图数、正文摘要、共享提示及恢复/永久删除。原项目被删除后，恢复为独立案例。

共享提示只统计本卡独立图片中被其他正常或回收站案例使用的张数；同案重复不算共享，草稿另参与文件保护。软删及永久删除均先处理当前编辑与该案例任何未结束草稿，包含干净影子稿；明确放弃草稿后，永久删除仍需单独二次确认。草稿可恢复继续编辑，回收站目标可作为新草稿继续。失败保留卡片、输入和可重试操作。

永久删除只在事务中删除案例关系、更新候选状态。资源协调锁按“编辑会话 → 资源 → DB”顺序覆盖导入准备、资产登记、关系变更及 GC；本运行导入资产与输出快照原图保留至 Store 结束，覆盖导入登记到草稿挂接的空窗，队列取消或完成不释放输出保护。保留 A–C 的 13px Dock、连续粘贴、焦点及 Esc 行为。

GUI 在启动、接受任务前执行统一候选清理，重新查正式/回收站/草稿引用。Windows 持有剪贴板读取锁直至清理结束；CF_HDROP 仍引用候选、剪贴板忙或解析不可靠时继续保留。按数据库已登记路径定点校验管理目录及链接边界后删文件，失败保留 pending 下次重试。候选文件已缺失可收尾；相同 hash 再导入会重建原图。**孤立 staging 和未登记原图保守保留，本阶段不扫描目录或清理磁盘全部孤儿。** 外部软件没有统一接收回执，自动化只证明本应用可观察引用和派发行为。

离线 `--check` 继续校验完整性、外键及所有已登记原图 hash。仅无任何案例/草稿引用、pending 且原图确实缺失时，成功返回 `recoverablePendingMissing`/IDs，`originalHashes` 明确为 `ok_except_recoverable_pending_missing`，供 GUI 启动收尾；有引用或 live 缺图、已有文件损坏仍失败。全量编辑恢复资产投影只含有引用资产，孤儿缺图不阻断有效草稿。

新增模块为 `src-tauri/src/trash.rs`、`gc.rs`、`clipboard_gc.rs`、`trash_commands.rs`；UI 为 `src/library/TrashCard.tsx`、`TrashDialogs.tsx` 与局部 `src/library-d.css`。报告和实际 React 截图见 `reports/stage-d/`。自动化只使用自建临时合成库；Windows GUI、真实剪贴板及外部应用接收待人工验收。最终 GUI 由主控通过 Tauri CLI 构建至 `src-tauri/target-stage-d`，保留 C 产物；开发阶段不提交、不推送、不自动合并 main，不进入 E/F。

```powershell
pnpm test
pnpm build
node scripts/check-ui-build.mjs stage-d
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib --bin prompt-shelf-poc --test store_cli --test scope_refresh --target-dir src-tauri/target-stage-d
# 测试桥 JSON 编译输出与 stderr 分开保存；PLAYWRIGHT_MODULE 可指向现成安装。
cargo test --manifest-path src-tauri/Cargo.toml --locked --test browser_store --no-run --message-format=json --target-dir src-tauri/target-stage-d 1> reports/stage-d/browser-bridge-build.jsonl 2> reports/stage-d/browser-bridge-build.txt
pnpm test:trash
$env:PROMPT_SHELF_TEST_PHASE = 'stage-d'
pnpm test:catalog
```

## E ZIP 备份与整体恢复

revision `4.4-e`，schema 保持 2。主库顶部“备份与恢复”打开 V5 弹窗：导出前先确认当前草稿已落盘，不强制正式保存；恢复预检后用严格 2×2 显示正常案例、回收站、标签、项目的“当前 → 备份”，草稿单列摘要。整体恢复替换当前资料库，保留上一完整库及其草稿；当前编辑须先保存/继续/放弃，预览后数据或草稿发生变化时更新计数并再次确认。文件名默认带日期时间，已有备份不覆盖；常见错误用中文说明。

备份通过已有 rusqlite Online Backup API 包含 WAL 中已提交内容；仅在临时快照清除无引用资产行及候选状态，原图按有效 case_assets（含回收站）和 draft_assets 去重，以原字节写入 ZIP。包含分类、快捷顺序、词典、偏好、正式案例及草稿；不含缩略图、日志或临时文件。清单记录格式/应用/schema、日期、数量、各文件大小和 SHA-256。打包流式执行，不在整个 ZIP 阶段持 DB 锁；显示字节/文件进度并可取消。进入最终发布或切库短提交后明确禁用取消，避免已接受取消却仍提交。

恢复入口一次验证 ZIP 条目/重复/链接/路径、版本、实际解压字节与可用空间、完整文件集合及 hash、图片格式/尺寸、数据库结构/完整性/外键和全部引用。只对临时库执行本应用已知 v1→2 迁移，不执行备份自带 SQL。预检成功产生内部 `PreparedRestore`，确认复用该对象；取消预览仅清理该已知临时目录。生产恢复不依赖 D 的宽松候选检查来代替预检。

GUI 和 CLI 共用 `LibraryHost` 的根级锁及 active/previous 解析。首次兼容 A–D 平面目录，不搬移、不覆盖旧库；恢复后同根 `storage/<uuid>/` 保存整套 DB+assets，先同步 `active.previous.json`，再同目录原子替换 `active.json`。切换关闭旧 DB，旧目录保留，不在切换时删除 D 输出原图。指针坏或当前库坏时验证上一库回退且增加 generation；均无效时报恢复错误，不自动建空库。已有空/零版本 DB 也不会被当成首次启动初始化。

维护门禁等待导入 worker 和队列实际结束，阻止新业务写入/导入/粘贴启动，普通读取可继续；已启动队列的首次派发记账允许收尾。锁顺序为 Host → 编辑会话 → 资源 → DB。IPC、编辑和缩略图授权都绑定真实 generation；恢复后撤销旧库授权、保留新库已授予文件，并重建两窗缓存/编辑/选择。旧 generation 迟到响应、写入和队列事件不能影响新库。后台 worker 才拥有维护和任务生命周期，取消/关闭前端不会提前释放。

旧 D 程序不理解 E 的 active 布局；schema 同为 2 不等于旧程序能正确打开 E 数据根。指针只提供上一库回退，不提供历史版本管理；更旧及未激活 storage 目录保守留存，不扫描清理。不要用旧 D 程序操作已切库的 E 根目录。

新增依赖锁定为 [zip 8.6.0](https://docs.rs/zip/8.6.0/zip/)（MIT，仅 Deflate 读取能力，导出 Stored）和 [fs2 0.4.3](https://docs.rs/fs2/0.4.3/fs2/fn.available_space.html)（MIT/Apache-2.0，用于可用空间查询），未升级原有依赖。新模块：`src-tauri/src/backup.rs`、`backup/`、`storage_host.rs`、`storage_host/`、`backup_commands.rs`；前端 `api.ts`、`LibraryGenerationBoundary.tsx`、`library/BackupDialog.tsx` 与局部 `library-e.css`。

报告见 `reports/stage-e/`。ZIP64 已验证格式结构/大文件选项，未运行实际超过 4GiB 的实包。Windows 临时文件系统切换和自建 worker 强杀后重开已测，不代表真实断电；系统剪贴板、GUI/DPI 和真实目标应用接收仍待人工验收。最终 release 由主控使用 Tauri CLI 构建到 `src-tauri/target-stage-e`；不覆盖 D，不进入 F，不提交或推送。

```powershell
pnpm test
pnpm build
node scripts/check-ui-build.mjs stage-e
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib backup::tests --target-dir src-tauri/target-stage-e -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib storage_host::tests --target-dir src-tauri/target-stage-e -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --locked --bin prompt-shelf-poc --target-dir src-tauri/target-stage-e -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --locked --test backup_cli --test scope_refresh --target-dir src-tauri/target-stage-e -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --locked --test browser_store --no-run --message-format=json --target-dir src-tauri/target-stage-e 1> reports/stage-e/browser-bridge-build.jsonl 2> reports/stage-e/browser-bridge-build.txt
# PLAYWRIGHT_MODULE 可指定现成 Playwright 包；需已安装 Edge。
pnpm test:backup
```
