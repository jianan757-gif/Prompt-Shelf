# Prompt Shelf · 4.4 B2 R2 编辑安全与图片交互

沿用 Tauri 2 + Rust + React/TypeScript、阶段 A 底座和已验收 B1 UI。B2 增加 SQLite 自动草稿、异常恢复与未保存处理，承接 B1 的案例内联编辑、图片操作和双窗刷新。停止在 B2，不扩展 C–F。

## 构建与验证

环境：Windows、Node 24、pnpm 11、Rust MSVC（当前 1.98.1）、VS 2022 Build Tools、WebView2。锁文件为 pnpm-lock.yaml 和 src-tauri/Cargo.lock。

```powershell
cd poc
pnpm install --frozen-lockfile
pnpm test
pnpm build
$env:CARGO_TARGET_DIR = Join-Path $PWD 'src-tauri/target-stage-b2-r2'
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --no-bundle -- --locked
cargo build --manifest-path src-tauri/Cargo.toml --release --bin prompt-shelf-store --locked
```

两个 exe 位于 target-stage-b2-r2/release/：prompt-shelf-poc.exe（主程序，文件名保留）、prompt-shelf-store.exe（离线工具）。旧阶段 A、round7 等目录不覆盖。reports/b2-r2/ 留本机测试、构建与哈希；编译/Mock 验证不代替真实目标接收和 Windows UI 手测。

## 数据目录与启动

生产 application identifier 为 io.github.jianan757-gif.promptshelf，与旧 local.promptshelf.poc 隔离。默认使用新标识的 Tauri 本地应用数据目录，首次为空库。不扫描 manifest、演示案例或 PROMPT_SHELF_MANIFEST，不自动迁移旧 PoC 数据。

GUI 仅支持可选 --data-dir <directory>，测试/开发指定独立目录：

```powershell
& './src-tauri/target-stage-b2-r2/release/prompt-shelf-poc.exe' --data-dir 'D:/PromptShelf-test/library-a'
```

主程序先初始化官方 single-instance 插件，再运行其他插件和 setup。重复启动不执行导入、数据打开、热键注册或目标改绑。单库单连接受 Mutex<Connection> 保护，SQL/文件工作在 blocking worker 中进行。Store 全生命周期持有 .library.lock 的 OS 锁，离线工具与 GUI、两个离线工具不能同时打开同一目录；进程退出由 OS 释放，不删除锁文件。

```text
<data-dir>/
  library.db                     SQLite schema v1（WAL/FULL，FK=ON）
  assets/<sha256>.<png|jpg|webp>   不可变原始字节
  cache/thumbs/<hash>-180-v1.png  独立缩略图，小图不放大
  staging/                       同卷临时导入文件
  logs/poc.jsonl                 运行日志
  .library.lock                  库占用锁
```

路径经 DataPaths 统一解析，数据库只存相对原图路径。迁移在事务中更新 user_version；未来版本、非空未版本化数据库和迁移失败均报错，不清库重建。A 没有 storage-id、active.json、切库或恢复机制。

## 显式开发样本导入与离线检查

prompt-shelf-store 是独立 console binary，无 Tauri Builder、窗口、托盘、热键或系统输入。必须提供 --data-dir，可附 --import-manifest <path> 和 --check。每次成功均执行完整性/外键/原图哈希检查，输出一行 JSON；失败非零退出。

```powershell
& './src-tauri/target-stage-b2-r2/release/prompt-shelf-store.exe' --data-dir 'D:/PromptShelf-test/library-a' --import-manifest '../fixtures/100x500/manifest.json' --check
# 导入进程退出后，以另一进程重开
& './src-tauri/target-stage-b2-r2/release/prompt-shelf-store.exe' --data-dir 'D:/PromptShelf-test/library-a' --check
```

先关闭持有同目录的程序；CLI 仍检测占用并拒绝打开。导入只允许空库，使用原 schema-v1 fixture 格式；original/thumb 必须是 manifest 根内存在的相对文件且互不混用。旧 thumb 只作兼容输入验证，正式缩略图从新保存原图重建。旧 ID 映射为新 UUID；同 hash 一资产/一原图，同案多位置、效果图/参考图与跨案共享保留为独立引用行。

流式复制/哈希 → 同卷 staging 校验格式/解码/哈希并同步文件 → 不覆盖落入 assets → 独立缩略图 → 一个事务提交全部资产/案例/分类/图片引用。失败不留下半写案例；中断最多留下未引用的完整原图/缓存，A 不自动物理清理。源文件不重编码、移动或覆盖。增量 Store::import_image 供 B 接入，仅登记图片，不创建案例或草稿。

## 资产与显示边界

PNG/JPEG/WebP；单源文件上限 512 MiB，解码宽高各上限 32768、分配预算 512 MiB。缓存高约180px、小图不放大、按比例 PNG RGBA；应用 EXIF 方向并传递 ICC profile。合成 alpha/方向/ICC 元数据测试不代表真实相机和显示器色彩验收。

缺失/损坏缓存可重建；重建前校验原图 hash，不从损坏原图生成该 hash 的预览。有效缓存命中不全量哈希原图。get_library 与启动共用 authorize_thumbnails，只对已验证缓存文件逐个 allow_file；不开放原图目录、邻文件或整盘。原图仅由 Rust 粘贴链路读取。

## 使用与本阶段限制

在目标输入框按 Ctrl+Alt+1/2/3 呼出；Enter/文字、单图、参考图队列仍使用原请求保护。面板粘贴后可见置顶、目标持焦，搜索/筛选和轨道位置保留。活动 Esc 仅取消，释放后闲时 Esc 收起并注销；隐藏后不持有全局 Esc。done/total 只表示已触发，不代表目标收到或上传完成。

A 的接入冒烟覆盖正式库快照到既有文本/图片准备，不发送系统粘贴。真实目标、GUI 重复启动、实际 DPI 与高分辨率/真实 ICC 素材仍需人工检查。主库已接手动保存；收藏、最近使用等业务仍留后续阶段。

## 目录与依赖

```text
src/                            现有 React UI、搜索、图片轨道
src-tauri/src/paths.rs           平面路径
src-tauri/src/store.rs           SQLite、导入、DTO 查询
src-tauri/src/migrations/        事务迁移
src-tauri/src/assets.rs          原图与可重建缩略图
src-tauri/src/scope.rs           缓存文件授权
src-tauri/src/bin/               离线 CLI
src-tauri/src/platform/          保留的系统粘贴适配
src-tauri/src/queue.rs,escape.rs  保留的队列和 Esc
src-tauri/tests/                 CLI 真实进程、scope MockRuntime
reports/stage-a/                 本阶段证据
```

新增依赖：rusqlite 0.41.0（bundled/backup，MIT）、libsqlite3-sys 0.39.1 自带 SQLite 3.54.1（public domain）；官方 tauri-plugin-single-instance 2.5.4（MIT/Apache-2.0）、tempfile 3.27.0（MIT/Apache-2.0），UUID 复用锁定的 1.x（MIT/Apache-2.0）。已有 image、sha2、Serde 等继续使用，实际版本见锁文件和 CLI sqliteVersion。官方依据：[单实例](https://v2.tauri.app/plugin/single-instance/)、[SQLite 版本](https://www.sqlite.org/changes.html)、[rusqlite](https://docs.rs/rusqlite/)。

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
