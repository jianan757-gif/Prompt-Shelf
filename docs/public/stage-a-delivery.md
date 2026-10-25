# Prompt Shelf 4.4 阶段 A 交付说明

日期：2026-10-25。范围：数据与资产底座；不进入 B1。基线为 `main` / `v0.1.0` / `dbccd3441c120169870c2f2406f187db46b45322`，本轮改动未提交、未推送。

## 实现

- 修订原实施计划：A 使用统一 DataPaths 平面目录；完整恢复切库留 E；缩略图按文件动态授权；B 内部分 B1/B2。
- SQLite schema v1：案例、项目、标签、图片、关联、草稿及设置等必要表和索引。外键、唯一约束、不可变原图字段约束在数据库执行；迁移在事务中完成。未来版本或无版本的非空数据库明确拒绝，不清库。
- 单连接 Mutex，阻塞读写放入 blocking 任务；没有 ORM、连接池或事件总线。WAL + FULL；数据目录锁覆盖 GUI 与离线工具，官方 single-instance 插件拦截重复 GUI 实例。
- 原图只读复制到同卷暂存文件，SHA-256 校验、解码后按哈希落盘，数据库再提交引用。PNG/JPEG/WebP；同图共享原文件，关联行保留独立顺序，允许重复引用。源图不压缩、不改写。
- 缩略图独立缓存，保持比例与透明度、应用 EXIF 方向、保留 ICC 字节；缓存缺失或损坏可重建。原图不通过 asset protocol 暴露；启动及 `get_library` 刷新逐文件授权新增缩略图。
- 正式启动读取 SQLite，首次为空库。保留旧 manifest 解析器用于显式、仅空库的离线样本导入；不再自动加载 JSON 或 demo。数据路径相对库根存储。
- 现有只读界面通过兼容 DTO 接入；粘贴前从 SQLite 取案例快照，继续交给原队列和平台实现。没有新增编辑、草稿恢复、回收站或备份业务。

## 关键代码

| 位置 | 职责 |
|---|---|
| `poc/src-tauri/src/paths.rs` | 唯一数据目录入口、相对路径与文件范围校验 |
| `poc/src-tauri/src/migrations/001_initial.sql`、`store.rs` | schema、事务迁移、引用约束、导入与查询 |
| `poc/src-tauri/src/assets.rs` | 稳定复制、去重、原图保护、缩略图生成/重建 |
| `poc/src-tauri/src/scope.rs` | 共用现有 asset protocol 的逐文件授权 |
| `poc/src-tauri/src/main.rs` | 单实例、Store 接入、异步查询与粘贴快照 |
| `poc/src-tauri/src/bin/prompt-shelf-store.rs` | 不启动 GUI 的显式开发导入/完整性检查 |
| `scripts/start-stage-a.ps1` | 独立测试库启动；不关闭旧进程、不覆盖非空库 |

新增直接依赖为 rusqlite（bundled SQLite）、uuid v4、tempfile、官方 single-instance 插件；沿用 image、sha2、Tauri、Tokio。版本以 Cargo.lock 为准。

## 验证与退出判定

| 验证项 | 结果及方式 | 本地证据 |
|---|---|---|
| SQLite/文件及必要回归 | Rust harness 47 passed / 0 failed：lib 14、main 30、真实离线进程 2、scope 1。悬空链接测试分支因权限 1314 未执行，不能将 47 视为全部分支实测 | `poc/reports/stage-a/rust-tests.txt` |
| 数据完整性 | 空库/重开、外键与唯一性、跨案及重复引用、坏图/SQL 失败回滚、迁移失败保留、未来 schema 拒绝、同库占用拒绝通过真实 SQLite/文件/CLI 测试 | 同上，测试源码 `store/tests.rs`、`tests/offline_process.rs` |
| 图片与动态授权 | 原图字节不变、alpha、合成 EXIF/ICC、缺失/损坏缓存重建通过；Tauri MockRuntime 下新增缩略图允许，原图/邻文件/数据库拒绝 | 同上，`tests/scope_refresh.rs`；不代表真实 WebView2 加载验收 |
| 前端与构建 | 前端 15/15；TypeScript/Vite 构建通过 | `poc/reports/stage-a/frontend-tests.txt`、`frontend-build.txt` |
| 既有实现保护 | Windows 平台、平台接口、队列、Esc、App/轮播及样式、旧 round7 exe 共 11 项 SHA-256 未变；main.rs 的数据接入有修改 | `evidence/stage-a/preserved-check.json` |
| 独立源码复核 | 悬空链接检查已修复并静态复核；没有扩展到 B～F 业务 | `evidence/stage-a-independent/review.md` |
| Windows release | GUI 与离线 CLI 均构建成功；两个程序的 SHA-256 已记录，GUI 未运行 | `poc/reports/stage-a/build.json`、`windows-release.txt`、`cli-release.txt` |
| 100/500 独立离线进程 | 空库→显式导入→另一进程重开→拒绝重复导入→再次检查，全部符合预期；100 案例、500 独立资产、501 图片引用；500 源图哈希未变，数据库/外键/存储原图校验通过 | `evidence/stage-a/offline-20261025-153538-4488496/summary.json` 及同目录 stdout/stderr |

独立导入用时约 11.59 秒，重开并完整检查约 0.37 秒，均为单次离线观测，不是 GUI 启动耗时或 P95，也不据此判资源达标。可公开的简明结果见 [`../validation/stage-a-results.json`](../validation/stage-a-results.json)。

**A 退出判定：代码实现与自动/离线验证已完成；完整阶段退出仍待下面列出的 Windows 人工接入冒烟，当前不宣称全部退出标准已通过。** 没有进入 B1，没有提交或推送本轮改动。

**证据边界**：Rust/文件/SQLite 及离线进程测试不等于 WebView2 实机或目标软件收件测试。未使用 Computer Use，未启动 GUI，未注入系统输入。新版本的空库 GUI、重复 GUI 启动及 SQLite 接入后的文字/单图链路仍需用户人工冒烟；原 4.3 验收结论保留，不把它冒充本轮新链路验收。

## 留待后续

- 导入失败可能保留尚未引用的原文件/缓存，不产生半条已保存案例。A 不主动清理这些文件；引用审查及安全清理在 D。
- 应用自身不修改已导入原图；不承诺检测外部程序对数据目录的每一次改写。原图完整性可用离线 `--check` 检查，本轮不额外改造原粘贴队列。
- B1 才接入正式编辑和用户导入入口；B2 完成草稿与关闭保护。当前草稿表仅是 schema，没有草稿业务。
- 完整切库、旧资源授权撤销、旧库回退随 ZIP 恢复在 E 实施。
- 合成 ICC/EXIF 测试只证明元数据及方向处理；真实照片颜色呈现和目标应用接收仍按计划补验。资源/P95 基线留相关阶段，不由本次导入耗时推定。

## 人工冒烟入口

先从托盘退出旧版，再在仓库根目录执行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\scripts\start-stage-a.ps1" -Fixture synthetic
```

该入口持久使用 `evidence/stage-a/manual-synthetic`；初次显式导入 100/500，重开不重复导入。

1. 热键呼出，确认文字及缩略图正常；向专用测试输入框贴一次文字和单图，面板常驻、Esc 可收起。
2. 托盘退出后运行同一命令，确认案例/图片仍在。
3. 运行中直接再次打开 `poc/src-tauri/target-stage-a/release/prompt-shelf-poc.exe`，确认不新增托盘或抢热键；启动脚本本身会拒绝重复启动，不能代替该插件验收。
4. 托盘退出后用 `-Fixture empty` 验证独立空库可以启动/重启；此参数不清空任何已有库。

完成这些 A 接入检查即可，无需重测整个 PoC；本轮停在 A。
