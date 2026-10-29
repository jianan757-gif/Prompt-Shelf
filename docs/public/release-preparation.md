# 最终发布准备报告

F（8358d6d）已通过用户验收及独立审核，保留完整历史合并 main 于 2915ed8；baseline-stage-f 指向 8358d6d，均已推送。补测与说明位于 codex/release-preparation。本轮不开发新功能，不改变 V5.0 UI、schema 2 或 A–F 行为，不创建 GitHub Release。

## 当前结论

- 已验收的 GUI 与 NSIS 安装包逐字节核对通过，不重新构建它们；版本仍为候选版 0.2.0-beta.1。
- 卸载“主动勾选删除应用数据”补测通过：用户明确确认“已勾选并完成卸载”；卸载后 SQL 全表内容、草稿、回收站及 500 张原图 SHA-256 与卸载前完全一致，schema 2、完整性和外键检查通过。前后指纹文件 SHA-256 同为 `3D8F46B2DCC5C8E6ADDCAF72D0CE15727C7188F8BB8DCED1233D7939C2AEE770`，证据见 docs/validation/release-preparation/uninstall-retention.json。用户动作确认与自动数据核验分别记录。
- 四种运行状态资源、正常退出、唤醒/搜索响应证据已齐。用户授权本次使用 Computer Use 切换独立测试库窗口，托盘正常退出由用户操作；数据由既有采样器和原生应用埋点记录。
- 只修正采样器一处 CPU 取整错误，并整理辅助脚本/离线 CLI；产品源码、UI、依赖及 schema 2 未改动。最终准备报告供审核，不自动发布。

## 实测性能基线

Windows 11 22631、i5-12600K、16逻辑处理器、31.75 GiB RAM、平衡电源模式。独立合成库100案例/500原图；使用已有 WebView2 缓存，不是干净机器基准。进程树包含已核对父子关系的 WebView2；未采 GPU。

| 状态 | 采样 | 私有内存 P95 MiB | 工作集 P95 MiB | CPU平均（整机100%） |
|---|---|---:|---:|---:|
| 初始隐藏 | 60次/60.7秒 | 266.98 | 412.59 | 0.0760% |
| 快捷面板可见 | 60次/60.6秒 | 221.78 | 428.65 | 0.2894% |
| 仅主库可见 | 60次/60.7秒 | 341.05 | 591.36 | 0.0734% |
| 主库关闭、面板隐藏，静置30秒后 | 60次/60.7秒 | 265.84 | 456.73 | 0.0033% |
| 正常退出后末段 | 连续23点/约22.3秒 | 0（进程已不存在） | 0 | 0 |

唤醒专项重复10次，P50 65.59ms、P95/最大90.77ms；完整首轮会话保留14次快捷键呼出数据。搜索使用原生窗口的搜索框，按 `001`/`摄影`/清空重复10轮，30个有效样本，P50 27.60ms、P95 50.40ms、最大54.10ms；测量输入至 React 结果提交及 RAF，不包含 Computer Use 调用延迟。原生主库首轮创建仅1次409.50ms，属于单次观察，不能据此宣称主库P95。对应有效样本没有未可交互/被取代记录。

这些是描述性基线，不设通过阈值，不推断长期无泄漏。主库与关闭后采样之间有人工中断/最小化，差额不单独归因于关闭动作；工作集之和可能重复共享页。窗口状态经原生界面观察，第一段隐藏由用户启动；响应证据来自首轮PID36356，CPU修正后四状态来自PID34864，同一GUI哈希和测试库。

退出证据采用用户首次明确确认的正常托盘退出（PID36356）。120.2秒过渡采样含等待用户操作，末段23点的全部已跟踪进程持续为零；退出后CPU为零由“无剩余进程”成立，不依赖旧CPU取整计算，不把过渡全程P95当作退出后占用。CPU补采启动的PID34864仍等待用户再次托盘退出；其120秒追加观察未发生退出，不计为第二次退出成功。它不替代首轮已完成的真实退出证据。

**采样器修正：** `[Math]::Max(0,...)` 在 PowerShell 绑定了整数重载，低CPU值会取整。改为 `Max(0.0,...)`；直接验证0.2修正前为0、修正后为0.2，并重采上述四段。旧CPU均值（含F历史“0%”）不能视为精确值；旧内存、进程数和响应记录不受影响。旧文件保留无效标记，正式表使用 `final-corrected-*` 数据；不把旧取整结果混入新基线。

证据：`resource-summary.json`、`resource-samples.csv`、`final-corrected-*.json`、`response-events.jsonl`、`response-baseline.json`、`warm-wake-baseline.json`、`cpu-sampler-correction.json`，均在 docs/validation/release-preparation/。

## 补测复现入口

**一、卸载复选框。** 打开 `evidence/release-preparation/installed-validation/uninstall.exe`（产品名应为 Prompt Shelf F Validation）。主动勾选“删除应用数据”，完成卸载；保留资料提示点确认。告诉主控是否确实勾选。主控随后运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-uninstall-retention.ps1 -Step Verify
```

只使用 `LocalAppData/io.github.promptshelf.stagef.validation` 的已标记测试库。只读比对数据库全部表的内容、草稿、回收站引用及原图 SHA-256，另做 F CLI 完整性、外键及原图检查。资料丢失时报失败，不创建空库掩盖问题。文件检查不能证明用户勾选了复选框，必须另留用户操作确认。

**二、性能采样。** 先从托盘正常退出当前 Prompt Shelf，然后在项目根目录运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\measure-final-baseline.ps1
```

脚本启动独立合成测试库 100 案例/500 图片，不访问个人资料。按终端提示操作：初始隐藏 → 打开快捷面板 → 打开主库并隐藏快捷面板 → 关闭主库并隐藏面板（静置30秒）→ 响应操作 → 托盘退出。每个状态采样60秒；空闲采样期间不要操作应用。

响应操作：按 Ctrl+Alt+1 / Esc 重复10次；在快捷面板搜索框中，将内容替换为 `001`、`摄影`、清空，重复10轮，每次等结果显示再继续。返回终端按回车，最后按提示从托盘退出。实际样本数、未可交互及被后续输入取代的样本会如实记录，不凑数量。

进程数据写入 evidence/resources；会话及响应汇总写入 evidence/release-preparation。使用既有进程树采样器，跟踪已发现的后代，即使根进程退出仍记录剩余进程。退出是否释放以末段进程数和内存原始数据核对，不只看主进程消失。CPU以全机逻辑处理器总量为100%；工作集相加可能重复共享页，不采 GPU。窗口状态由用户操作声明；采样值不自动成为“性能达标”结论。主库响应可能包含冷创建，不推测冷热分类。

本轮由主控按已观察的窗口状态逐段调用采样器；响应操作通过获授权的 Computer Use 完成，引导脚本未全程串行跑完，不将其完整交互流程记为独立测试通过。

## 候选产物及交付说明

| 产物 | 版本 / 大小 | SHA-256 |
|---|---|---|
| Prompt Shelf_0.2.0-beta.1_x64-setup.exe | 0.2.0-beta.1 / 4,454,625 bytes | 2BCA30D99AE05789CBDC6E8CF214A6DE4E753F49B48BD14BC2132BDF0964F2ED |
| prompt-shelf-poc.exe（用户界面名称 Prompt Shelf） | 0.2.0-beta.1 / 15,549,952 bytes | B7508445273A9F6E7AF647950665CA75F2644A0FDDD85586919CCFE64164373E |
| prompt-shelf-store.exe（辅助 CLI，本轮重建） | 0.2.0-beta.1 / 3,229,184 bytes | 7B545AFE258D95C725777D33278732FA01F31373D236524D91338E430D9E5DEA |

GUI 与安装包路径为 `poc/src-tauri/target-stage-f/release/` 及其中 `bundle/nsis/`；两者在 evidence/stage-f/candidate/ 另有同哈希副本。CLI 使用 release/ 中本轮重建版本。测试身份打包曾覆盖辅助 CLI，因此其本机哈希与首轮 F 记录不同；本轮用未改动的已审核源码单独重建，记录新哈希并复核 SQL/原图不变。不要用旧 CLI 哈希描述当前文件；不宣称不同构建具备逐字节可复现性。

升级与重装：正常退出程序后，以相同当前用户身份运行候选安装包；继续沿用 identifier `io.github.jianan757-gif.promptshelf` 与 LocalAppData 资料根，不手动删除该目录。资料与安装目录分离，卸载 hook 保留资料。旧 D 程序不理解 E/F 的 active 指针布局，不能直接打开恢复切换后的 E/F 资料根；代码回退不等于资料可交给任意旧程序。

本轮安装卸载验证仅用独立测试身份。已有 F 自动化安装/升级/卸载/重装通过记录仍有效；旧版本测试包为同一源码配置0.1.0，不是历史 E 安装器。所有产物未签名；缺少 WebView2 的安装、不同 Windows/DPI、真实断电、macOS 未验证。进度表示已派发，不等于目标应用收件成功；目标应用可能选择格式或转码。保留现有验证边界，不补造兼容性结论。

## 证据入口与待办

公开证据：docs/validation/release-preparation/；完整本机指纹和原始日志：evidence/release-preparation/、evidence/resources/。辅助脚本已完成 PowerShell/Python 语法检查；响应汇总器用合成日志验证会话过滤、分位数、缺失样本和被取代输入统计；现有采样器用自建临时 worker 验证退出后计数/私有内存归零。这些均不替代本轮应用的实际补测。

卸载复选框、四状态资源、首轮正常退出及响应证据已完成；资料复查100案例/500原图、完整性/外键/原图哈希通过。等待最终审核及候选发布批准；补采程序的再次退出属于待用户收尾，不冒充已验证。当前不发布稳定版、不创建 Release。
