# Prompt Shelf · 4.4 B1 主库 UI

沿用 Tauri 2 + Rust + React/TypeScript 工程，底座来自阶段 A，本轮仅还原主库浏览 UI。快捷面板、只读主库从 SQLite 兼容 DTO 读取；不实现 B–F 的编辑、草稿业务、回收站或备份。窗口几何、样式、轮播、粘贴、焦点、队列和 Esc 语义保留。

## 构建与验证

环境：Windows、Node 24、pnpm 11、Rust MSVC（当前 1.98.1）、VS 2022 Build Tools、WebView2。锁文件为 pnpm-lock.yaml 和 src-tauri/Cargo.lock。

```powershell
cd poc
pnpm install --frozen-lockfile
pnpm test
pnpm build
$env:CARGO_TARGET_DIR = Join-Path $PWD 'src-tauri/target-stage-b1-ui'
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --no-bundle -- --locked
cargo build --manifest-path src-tauri/Cargo.toml --release --bin prompt-shelf-store --locked
```

两个 exe 位于 target-stage-b1-ui/release/：prompt-shelf-poc.exe（主程序，文件名保留）、prompt-shelf-store.exe（离线工具）。旧阶段 A、round7 等目录不覆盖。reports/b1-ui/ 留本机测试、构建与哈希；编译/Mock 验证不代替真实目标接收和 Windows UI 手测。

## 数据目录与启动

生产 application identifier 为 io.github.jianan757-gif.promptshelf，与旧 local.promptshelf.poc 隔离。默认使用新标识的 Tauri 本地应用数据目录，首次为空库。不扫描 manifest、演示案例或 PROMPT_SHELF_MANIFEST，不自动迁移旧 PoC 数据。

GUI 仅支持可选 --data-dir <directory>，测试/开发指定独立目录：

```powershell
& './src-tauri/target-stage-a/release/prompt-shelf-poc.exe' --data-dir 'D:/PromptShelf-test/library-a'
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
& './src-tauri/target-stage-a/release/prompt-shelf-store.exe' --data-dir 'D:/PromptShelf-test/library-a' --import-manifest '../fixtures/100x500/manifest.json' --check
# 导入进程退出后，以另一进程重开
& './src-tauri/target-stage-a/release/prompt-shelf-store.exe' --data-dir 'D:/PromptShelf-test/library-a' --check
```

先关闭持有同目录的程序；CLI 仍检测占用并拒绝打开。导入只允许空库，使用原 schema-v1 fixture 格式；original/thumb 必须是 manifest 根内存在的相对文件且互不混用。旧 thumb 只作兼容输入验证，正式缩略图从新保存原图重建。旧 ID 映射为新 UUID；同 hash 一资产/一原图，同案多位置、效果图/参考图与跨案共享保留为独立引用行。

流式复制/哈希 → 同卷 staging 校验格式/解码/哈希并同步文件 → 不覆盖落入 assets → 独立缩略图 → 一个事务提交全部资产/案例/分类/图片引用。失败不留下半写案例；中断最多留下未引用的完整原图/缓存，A 不自动物理清理。源文件不重编码、移动或覆盖。增量 Store::import_image 供 B 接入，仅登记图片，不创建案例或草稿。

## 资产与显示边界

PNG/JPEG/WebP；单源文件上限 512 MiB，解码宽高各上限 32768、分配预算 512 MiB。缓存高约180px、小图不放大、按比例 PNG RGBA；应用 EXIF 方向并传递 ICC profile。合成 alpha/方向/ICC 元数据测试不代表真实相机和显示器色彩验收。

缺失/损坏缓存可重建；重建前校验原图 hash，不从损坏原图生成该 hash 的预览。有效缓存命中不全量哈希原图。get_library 与启动共用 authorize_thumbnails，只对已验证缓存文件逐个 allow_file；不开放原图目录、邻文件或整盘。原图仅由 Rust 粘贴链路读取。

## 使用与本阶段限制

在目标输入框按 Ctrl+Alt+1/2/3 呼出；Enter/文字、单图、参考图队列仍使用原请求保护。面板粘贴后可见置顶、目标持焦，搜索/筛选和轨道位置保留。活动 Esc 仅取消，释放后闲时 Esc 收起并注销；隐藏后不持有全局 Esc。done/total 只表示已触发，不代表目标收到或上传完成。

A 的接入冒烟覆盖正式库快照到既有文本/图片准备，不发送系统粘贴。真实目标、GUI 重复启动、实际 DPI 与高分辨率/真实 ICC 素材仍需人工检查。主库仍只读，收藏、最近使用等业务在后续阶段接入。

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

## 本轮 B1 UI 范围

独立 LibraryManager / CaseCard / LibrarySidebar / CatalogPanel 按冻结 V4.11 转为 React，沿用原型 SVG、尺寸与样式；V5.0 仅保留入口语义，不实现编辑、保存、草稿或回收站/备份业务。原型的演示条、重置与模拟中断工具不进入产品。

可用：SQLite 只读浏览、关键词搜索、集合/项目/标签筛选、常规网格/瀑布流、二级分类栏、单/双列分类、分类搜索/时间和名称排序、栏宽拖动、90px 图片与逐张箭头、词典规则预览和原生窗口控制。分类与日期/来源/回收站数量只读既有列，未标快捷的标签不会自动变快捷。

未开放入口：新建/编辑案例、收藏修改、来源外链打开、原图预览、分类新增/编辑/重命名/删除、快捷标签设置/排序、回收站、备份恢复。只显示轻提示，不写 SQLite，不假保存、不承诺草稿已保护。

本轮构建目录为 src-tauri/target-stage-b1-ui；同名 GUI/CLI 文件及 --data-dir / --import-manifest / --check 参数保持。主库与快捷面板在 main.tsx 显式分支加载各自 CSS；scripts/check-ui-build.mjs 检查实际发布入口的 CSS 预加载依赖；复现命令：先运行 pnpm build，再运行 pnpm check:ui-build。前端保留原测试并增加只读排序/日期检查，Rust 仅跑受影响 DTO 定向测试，不重复完整 PoC。

主控已完成 Windows 主要界面实测，2026-10-25 用户确认 B1 主库 UI 视觉验收通过，并补测确认窗口原生拖动、窄窗口适配和跨程序粘贴回归通过。以上补测属于用户实机验收，不是新增自动化结果；原型浏览器渲染图逐像素比较未完成。详见 ../docs/public/b1-ui-delivery.md。当前不继续编辑保存业务。
