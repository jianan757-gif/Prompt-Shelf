# F Windows 集成与交付准备

E 及 bb9f6d8 已通过审核，main 保留历史合并于 68d675a；baseline-stage-e 指向 bb9f6d8。F 分支 codex/4.4-f-delivery，候选版 0.2.0-beta.1；未经用户验收和独立审核，不合并 main、不发布稳定版。

**当前状态：用户反馈“已全部通过，推送吧”，F 人工验收清单 1–6 记为用户报告通过，提交开发分支供独立审核。** 未提供逐项截图、逐应用版本或额外资源采样文件，不补造明细；本次整理验收记录，不重复执行已完成测试。

## 交付范围

- Tauri 正式资源协议构建，包含 E 权限清理警告修复；产品名称 Prompt Shelf，清除用户可见 PoC 标识。
- 官方 NSIS 当前用户安装；保留原 identifier 和 LocalAppData 资料路径，SQLite schema 仍为 2。安装目录与资料目录分离。
- 工程内部仍保留 poc/、Rust crate 和 prompt-shelf-poc.exe 名称，避免不必要的全工程重命名；这不改变产品展示名称。
- 开始菜单/桌面启动、托盘及单实例复用；不新增登录自启、自动升级服务、云功能或 Mac 专属功能。

构建：根目录运行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\build-stage-f.ps1`。独立测试库启动：同方式运行 `scripts/start-stage-f.ps1 -Fixture empty` 或 synthetic；默认目录 evidence/stage-f/manual-*，重复启动保留资料。

## 本轮结果

| 验证 | 结果与证据（docs/validation/stage-f） |
|---|---|
| Rust 逻辑与授权 | 库 81、主程序 40、授权 2 项通过；队列平台模拟，不等于系统粘贴。rust-tests.txt |
| 前端与两窗 | 逻辑 30、实际 React 分类/编辑/同步 16、备份恢复 9、恢复警告 4 项通过；测试 IPC 连接真实 Store/Host。frontend-tests.txt、catalog-tests.txt、backup-tests.txt、cleanup-warning-tests.txt |
| 构建 | TypeScript/Vite、分窗样式检查、Tauri GUI/CLI release、NSIS x64 通过。production-build.json、packaged-css.json |
| 安装与保数据 | 独立应用身份实际静默安装 0.1.0、升级候选、卸载、重装均 exit 0；100 案例、500 原图、1 草稿/5 图片关系、回收站、分类、收藏/最近状态的 SQL 指纹与原图 SHA-256 不变。E CLI 创建库，F CLI 每步完整性/外键/原图检查通过。installer-results.json、data-retention.json |
| 资源基线 | 100/500 隐藏状态 60 秒、60 次采样：进程树私有内存 P95 244.15 MiB，工作集 P95 398.69 MiB，采样 CPU 平均为 0%。含后代进程，不含 GPU；采样期间另有构建工作，非受控空闲系统。hidden-resource-baseline.json |
| 搜索计算基线 | Node 执行生产 filterCases，100 案例、600 次采样 P95 0.95ms；不含 IPC、React 绘制或图片读取。search-computation.json |
| Windows 界面及真实目标 | 用户报告清单 1–6 全部通过；未提供逐应用明细。Computer Use 已停止，后续不再使用，用户实测与自动化证据分别记录。 |

安装测试使用 `io.github.promptshelf.stagef.validation`，不是个人资料身份。旧测试包为同一 F 源码配置 0.1.0，验证版本升级流程，并非历史 E 安装器（E 没有安装包）。正式 identity 保持 `io.github.jianan757-gif.promptshelf`；未在真实用户库执行安装升级。静默测试没有勾选删除资料，hook 执行先于删除条件已检查；用户未单独提供该复选框的操作明细，不将其记为独立专项实测。测试安装已最终卸载，测试资料与日志保留。

候选 EXE 与安装包本地位于 `evidence/stage-f/candidate/`，生产输出位于 `poc/src-tauri/target-stage-f/release/`。安装包 `Prompt Shelf_0.2.0-beta.1_x64-setup.exe` 约 4.25 MiB，SHA-256 `2BCA30D99AE05789CBDC6E8CF214A6DE4E753F49B48BD14BC2132BDF0964F2ED`；GUI SHA-256 `B7508445273A9F6E7AF647950665CA75F2644A0FDDD85586919CCFE64164373E`。候选包未签名，未创建 GitHub Release。原始日志及完整逐图指纹在本地 evidence/stage-f、poc/reports/stage-f，不公开个人路径。

剩余事项：等待 F 独立审核。用户已确认下方清单通过；主库/面板/关闭后资源、唤醒及搜索端到端响应仍缺量化采样，性能阈值未批准，不声称全面性能达标。未测缺少 WebView2 的安装路径、不同 Windows/DPI 环境、真实断电及 macOS。F 用户验收依据本次反馈，不以此前 A–E 记录代替。

## Windows 人工验收清单

所有操作仅在独立测试库。测试前保留该测试库的 ZIP，不使用私人真实资料；不必为验收重新跑全部 PoC。

1. **安装与启动**：安装操作仅用独立 Windows 测试账户，或本机 `target-stage-f/release/bundle/nsis/Prompt Shelf F Validation_0.2.0-beta.1_x64-setup.exe` 的专用测试身份（测试库可能已有自动化样本，不要求清空）。检查产品名/版本、开始菜单/桌面快捷方式、托盘；重复启动只有一个运行实例，退出后可重开。正式 EXE 的界面测试用 `start-stage-f.ps1 -Fixture empty`，首次为空库。若安装受系统签名策略阻止，记录实际提示，不能视为已通过。
2. **快捷调用**：在专用记事本输入区绑定目标，Ctrl+Alt+1/2/3、文字多行、单图、多图顺序、进度和 Esc；面板保留检索，空闲 Esc 收起。ChatGPT/豆包按常用实际目标补测，记录 Web/客户端及结果。
3. **编辑与两窗**：新建/编辑、共享图片替换和排序、保存后两窗同步；未保存关闭/切换/取消的保存、继续、放弃；异常退出恢复草稿，正式旧版不被覆盖。
4. **回收站与恢复**：软删撤销、计数、恢复、共享引用提示、永久删除；备份后修改，再预检四格对比、取消/整体恢复、重启后检查正文/分类/顺序/草稿。
5. **升级/重装保数据**：仅对专用测试身份或测试 Windows 账户，保留有案例/草稿/回收站/共享图的库；升级、卸载后重装，核对记录、原图和启动路径。正式应用身份在个人账户中不用于自动破坏性测试。
6. **体验与资源**：100 案例/500 图，主库和快捷面板搜索与滚动；反复打开关闭主库、隐藏再唤醒及退出。资源数据先作为基线提交，未批准阈值不写“性能达标”。

## 验证边界

自动化逻辑、测试 IPC、Windows 原生 GUI、安装器测试身份、真实目标接收分别记录。编译通过不代表全部界面与跨程序功能通过；测试身份安装不代表真实用户资料上的升级已执行。macOS 不标通过。所有本机产物保留本地，不自动上传 GitHub Release。
