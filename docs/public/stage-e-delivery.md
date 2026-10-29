# E ZIP 备份与恢复交付记录

2026-10-29。状态：E 原交付及修复 bb9f6d8 均已通过用户验收及外部独立审核，正式结项。main 于 68d675a 保留完整历史合并 E；回退标签 baseline-stage-e 指向 bb9f6d8。D 已通过用户及独立审核，以 `5b92cd4` 保留历史合并 main；回退标签 `baseline-stage-d` 指向 `6e2d3e7`。E 分支为 `codex/4.4-e-backup`，已获批准进入独立 F 分支；本记录保留 E 当时的验证与产物边界，不发布稳定版。

## 审核后修正：恢复成功与权限清理警告分离

基于 `38691f7` 修复：新库已安装并初始化成功后，旧缩略图授权清理失败不再抛出恢复失败。恢复返回成功、新库继续使用；记录 `restore_scope_cleanup_warning` 日志，保留代际和原始错误。真正的恢复/初始化失败仍走原失败及回退路径。

主库与快捷面板复用现有提示区域显示“资料库已恢复成功；旧缩略图访问权限清理未完成，详情已记录日志。”警告随当前代际保存在运行状态中，两窗重挂载或重读状态不会丢失；后续正常恢复清除旧警告。日志持久保留，不增加数据库字段、依赖或自动重试机制。

故障注入仅用于独立临时资料库和测试桥，不是实机制造权限异常。修复的定向及回归证据见 [cleanup-warning](../validation/stage-e/cleanup-warning/)。本次未替换正在运行的 E 程序、未重新构建 release；下文生产产物及 108 文件源码清单均属于修复前已验收基线，不代表本次修复的可执行文件。实机复核需正常退出后重新构建。

| 本次验证 | 结果及证据（上述目录） |
|---|---|
| 恢复命令与取消/维护回归 | 3 项通过，包含新增故障测试的失败/正常两分支；新库保持活动，返回成功，日志及可重读警告一致。`backup-commands-regression.txt` |
| 缩略图权限回归 | 2 项通过，Tauri MockRuntime 验证新库授权保留及仅授权缩略图。`scope-refresh-regression.txt` |
| 实际 React + 生产 Store/Host、测试 IPC | 4 项通过：成功返回及新库数据、两窗重挂载提示、重新加载提示保留、正常恢复清除旧警告。`browser-targeted-final.txt`、`restore-cleanup-warning.png` |
| 前端逻辑、构建与样式隔离 | 30 项通过；TypeScript/Vite、CSS 检查通过。`frontend-tests.txt`、`frontend-build.txt`、`packaged-css-check.txt` |

复现：在 `poc/` 执行 `cargo test --manifest-path src-tauri/Cargo.toml --bin prompt-shelf-poc backup_commands --locked --target-dir src-tauri/target-stage-e`、同参数 `--test scope_refresh`（替代 `--bin … backup_commands`）、`pnpm test`、`pnpm build`、`node scripts/check-ui-build.mjs stage-e-scope-warning`。React 故障测试入口为 `tests/browser/restore-scope-warning.cjs`，先将 `cargo test --manifest-path src-tauri/Cargo.toml --test browser_store --no-run --message-format=json --locked --target-dir src-tauri/target-stage-e` 的标准输出保存到 `reports/stage-e-scope-warning/browser-bridge-build.jsonl`；使用现有 Playwright（可由 `PLAYWRIGHT_MODULE` 指定）及 Edge 运行。所有测试使用独立临时库。

## 范围与数据边界

完整 ZIP 备份、预检、V5.0 四格恢复对比和整体恢复。备份包含正式记录、回收站、草稿、分类、收藏、图片关系与配置；原图按 SHA-256 去重，缩略图为可重建缓存。恢复不是合并。

所有自动化、故障注入和人工验收均使用独立临时库。禁止指定个人真实资料目录。恢复后保留上一库，不能将“代码可回退”理解为任意旧程序都能读取新目录布局：旧 D 程序不认识 E 的 active 指针，即使数据库 schema 仍为 2。

## Windows 验收清单及结果

用户反馈：“1-5验收清单全部通过，6用自动化取消用例补正”。第 1–5 项记为用户实机验收通过；第 6 项免人工补测，沿用已有自动化结果，不记为人工通过。未提供逐项截图或目标应用明细，不补造逐应用结论。

第 6 项补证：[`archive-tests.txt`](../validation/stage-e/archive-tests.txt) 覆盖取消导出不发布/不覆盖、取消预检清理临时目录及取消/提交互斥；[`worker-lifetime-tests.txt`](../validation/stage-e/worker-lifetime-tests.txt) 验证取消后维护许可直到 worker 结束才释放；[`browser-tests.txt`](../validation/stage-e/browser-tests.txt) 验证实际 React + 生产 Store/ZIP 的取消、草稿保留、预览释放及损坏 ZIP 不改变当前库。以上均为此前已通过的隔离自动化，本次仅核对并关联记录，没有重复执行测试。

使用 `scripts/start-stage-e.ps1 -Fixture empty` 的独立测试库；重启仍使用同一目录。测试素材仅用非敏感样本。

1. **完整备份**：准备两条案例，共享一张图片，同案重复引用并调整顺序；填写多行正文、项目、标签、收藏，保留一条回收站案例和一份自动保护草稿。导出 ZIP，确认界面有进度和结果，原资料不变。
2. **预检与四格对比**：备份后修改当前案例，新增分类或案例；选择原备份。确认文件名、日期与四格“当前 → 备份”计数准确（正常案例、回收站、标签、项目），草稿显示在摘要。取消后当前资料不变。
3. **整体恢复**：确认恢复。检查正文换行、分类、收藏、图片顺序、共享图片、回收站与草稿；只有手动保存草稿才更新正式内容。备份后新增的资料不应被合并进入恢复结果。
4. **编辑保护与两窗同步**：有未保存编辑时发起恢复，分别检查继续编辑、保存及放弃路径；取消恢复不应丢失编辑。恢复成功后主库和快捷面板均显示新库，旧编辑器不能继续写入旧状态。
5. **重启与调用**：正常退出后重新启动同一测试库，恢复结果仍在；检查文本、单图及队列粘贴和 Esc，保持已有交互。
6. **取消与错误**：备份/预检期间取消，或选择损坏的测试 ZIP，当前资料仍可使用。若小样本处理太快无法手动取消，记录未人工覆盖，由自动化取消用例补证，不重复造大库。

原图哈希、ZIP 路径越界、缺图、未来 schema、空间不足及切换中断通过隔离自动化验证；交付时逐项列明真实文件测试或故障模拟，不要求用户操作真实资料或制造真实磁盘故障。

## 实现、验证与未完成项

关键代码入口：

| 模块 | 职责 |
|---|---|
| `poc/src-tauri/src/backup.rs`、`backup/validate.rs` | SQLite Online Backup、有效引用原图去重打包、清单/ZIP/数据库/原图预检，临时快照清理不影响在线库 |
| `poc/src-tauri/src/storage_host.rs` | 统一 GUI/CLI 资料库入口、根锁、维护模式、代际与修订、活动指针切换及回退 |
| `poc/src-tauri/src/backup_commands.rs` | 原生文件选择、后台任务、进度/取消、预览与恢复协调；复用既有队列和编辑保护 |
| `poc/src/api.ts`、`LibraryGenerationBoundary.tsx` | 两窗按资料库代际重新读取，拒绝旧会话写入与迟到结果 |
| `poc/src/library/BackupDialog.tsx`、`library-e.css` | V5.0 恢复四格对比、草稿摘要及操作反馈 |

沿用 schema 2，不新增业务表。新增 `zip 8.6.0` 与 `fs2 0.4.3`；SQLite backup、哈希、临时文件和文件对话框复用已有依赖。备份直接保存原图字节，不重新编码图片。目标 ZIP 已存在时拒绝覆盖，默认建议文件名包含时间。

证据目录：[stage-e](../validation/stage-e/)。分组存在交叠，不累计为独立测试总数。

| 验证方式 | 结果 | 公开证据 |
|---|---|---|
| 真实 SQLite/ZIP/临时文件 | 归档 10 项通过：含 WAL 快照、正文/分类/顺序/草稿/配置往返、共享原图 SHA-256、坏包/版本/路径/空间/取消 | `archive-tests.txt` |
| 资料库切换 | 10 个测试函数通过（含 1 个子进程入口）；Windows 在 previous/active 实际持久化节点结束自建 worker 后重开通过，旧库保留 | `host-tests.txt` |
| 队列与 IPC | 39 项通过；队列使用模拟平台。后台 worker 许可生命周期 2 项定向复跑通过，属于上述 39 项 | `queue-ipc-tests.txt`、`worker-lifetime-tests.txt` |
| CLI / 缩略图授权 | 真实 CLI 子进程 1 项、Tauri MockRuntime 授权 2 项通过 | `scope-cli-tests.txt` |
| 前端逻辑/构建 | 30 项通过；TypeScript、Vite 和分窗样式检查通过 | `frontend-tests.txt`、`frontend-build.txt`、`packaged-css-check.txt` |
| 实际 React 两窗 + 生产 Store/Host/ZIP，测试 IPC | 9 个场景通过；含备份前草稿保护、取消、旧文件不覆盖、预览重新确认、整库恢复、旧代际/迟到响应拒绝、子进程重启 | `browser-tests.txt`、[四格对比截图](../validation/stage-e/backup-restore-preview.png) |
| 源码冻结 | 108 份文件哈希已核对一致 | `source-manifest.json`（路径相对 `poc/`） |
| 独立复测 | 25 个报告测试项通过，含上述 Host helper；源码 108/108、保护基线 11/11 一致，无源码修改 | `independent-summary.txt` 及对应分组日志 |
| 生产构建 / CLI 冒烟 | Tauri GUI 和 CLI release 构建通过；独立空库首次打开及重开均通过完整性/外键检查，schema 2 | `production-build.json`、`gui-build.txt`、`store-build.txt`、`release-smoke.json` |
| Windows 原生 GUI / 系统粘贴 | 用户确认第 1–5 项通过；第 6 项以已有自动化补证，未记为人工通过 | 本次用户反馈；自动化证据见上方补证说明，未提供逐应用明细 |

初期测试曾遇到测试类型不匹配、MockRuntime 单测缺少 Windows Common Controls v6 清单导致进程启动失败，以及移动测试入口后的导入路径错误；已修正，相关组随后通过。完整过程日志留在本地 `poc/reports/stage-e/`，公开目录保留最终结果。

明确的验证边界：ZIP64 局部格式结构验证不代表实际导出超过 4 GiB；空间不足故障注入不代表实际填满磁盘；测试子进程终止不代表真实断电。旧库保守保留，不自动删除此前恢复目录；本阶段不增加历史库清理界面。

## 构建与交付入口

项目根目录运行 `scripts/build-stage-e.ps1`，产物位于 `poc/src-tauri/target-stage-e/release/`，保留 D 产物。GUI SHA-256：`2FD5F720085F9D78EC3B8D832012DDB218FF613B6A0635AB9CC94256D8F003F9`。完整本机构建记录在 `evidence/stage-e/`。PowerShell 5 将原生工具 stderr 进度显示为 NativeCommandError；本脚本以进程退出码判定，两次构建均为 0。

人工验收启动（默认独立空库；再次启动不会清空）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-stage-e.ps1 -Fixture empty
```

交付仅为开发分支源码、公开脱敏证据和本机非安装版程序。未经用户验收及独立审核，不将 E 标为正式结项。
