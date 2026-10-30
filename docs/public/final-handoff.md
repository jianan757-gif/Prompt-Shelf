# Prompt Shelf 最终交付说明

2026-10-30：A～F 开发及最终发布准备已通过用户验收和 ChatGPT 独立审核，正式收尾。最新审核提交 `2d614ad051ab740f24517e04694313b9cb11e568` 以合并提交 `060cb917e9cfd6acfcf7c0ff8d8e1eb0c57fbd85` 保留完整历史进入 main；回退标签 `baseline-release-preparation` 指向该审核提交。此前各阶段标签和验证证据保留。

当前版本为 **0.2.0-beta.1，Windows 内部测试候选版**，不是正式稳定版。本轮只做 Git 合并、文档收尾和已验收文件的只读哈希/版本核对；不重新构建，不访问私人资料库，不重跑 A～F 全量测试。V5.0 UI、schema 2、核心业务逻辑均未改动。

## 候选文件与校验

| 文件 | 本地位置（相对项目根目录） | SHA-256 |
|---|---|---|
| 安装包，4,454,625 bytes | `poc/src-tauri/target-stage-f/release/bundle/nsis/Prompt Shelf_0.2.0-beta.1_x64-setup.exe` | `2BCA30D99AE05789CBDC6E8CF214A6DE4E753F49B48BD14BC2132BDF0964F2ED` |
| GUI，15,549,952 bytes | `poc/src-tauri/target-stage-f/release/prompt-shelf-poc.exe` | `B7508445273A9F6E7AF647950665CA75F2644A0FDDD85586919CCFE64164373E` |
| 辅助 CLI，3,229,184 bytes | `poc/src-tauri/target-stage-f/release/prompt-shelf-store.exe` | `7B545AFE258D95C725777D33278732FA01F31373D236524D91338E430D9E5DEA` |

三个文件版本均为0.2.0-beta.1，与最终审核记录一致。[本轮校验记录](../validation/final-handoff/artifact-verification.json)。GUI及安装包另有同哈希本机副本，位于 evidence/stage-f/candidate/；辅助CLI以release/中的上表文件为准。工程目录/内部exe名保留poc字样，产品显示名称为Prompt Shelf。以后重新构建时须重新校验文件，不套用本表哈希。

安装前可在项目根目录执行：

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath '.\poc\src-tauri\target-stage-f\release\bundle\nsis\Prompt Shelf_0.2.0-beta.1_x64-setup.exe'
```

## 安装与使用

1. Windows x64环境运行上述NSIS安装包，按当前用户安装；通过开始菜单或安装快捷方式启动。WebView2已安装时复用；缺少时安装器沿用官方下载引导，需要网络，该缺失环境路径尚未实测。
2. 首次默认隐藏在托盘；Ctrl+Alt+1/2/3呼出全部/最近/收藏，或从托盘打开主库。直接启动正式EXE和安装快捷方式使用默认资料库，首次为空，不自动导入样本。
3. 在目标应用输入区先取得焦点，再按热键绑定目标；点击提示词/图片调用。快捷面板保留检索上下文，忙时Esc中止队列，空闲Esc隐藏。主库案例编辑需手动保存，自动草稿保护不等同于正式保存。
4. 开发验收入口 `scripts/start-stage-f.ps1 -Fixture empty` 使用 evidence/stage-f/manual-empty 独立库，和默认资料库分开；重启不清空。`--data-dir`仅用于明确指定资料根，不会自动迁移个人资料。

## 升级、卸载与重装

升级前从“备份与恢复”导出完整ZIP，再从托盘正常退出程序，使用相同Windows用户运行后续获批准的安装包。沿用现有应用标识与资料根，不手动删除数据目录。安装升级流程已用独立测试身份验证；测试旧安装包是同源码配置0.1.0，不是历史E安装器，不能推断任意历史版本都兼容。

卸载只移除程序，保留资料。专用测试身份主动勾选“删除应用数据”后，数据库全表、草稿、回收站和500原图指纹仍完整一致；同用户重装后继续读取原资料。安装目录不是资料根，不要为了重装清空LocalAppData中的资料。

## 备份与恢复

主库顶部“备份与恢复”→“导出完整备份”，保存独立ZIP。备份包含SQLite一致性快照、有效引用的去重原图、案例/分类/收藏/最近使用配置及草稿/回收站关系；缩略图缓存和日志不属于备份内容。备份不是一份直接复制的在线DB文件。

恢复时选择完整ZIP，等待预检，核对2×2“当前资料库→备份资料库”对比及草稿摘要，再确认整体恢复。**恢复替换整库，不进行数据合并**；恢复前先备份当前资料。损坏、缺图、哈希/版本等预检失败不覆盖当前库。恢复后两窗重读新库；草稿恢复进入编辑，仍须手动保存才成为正式内容。此前资料目录保守保留，没有历史库清理UI。

## 数据位置

默认资料根为 `%LOCALAPPDATA%\io.github.jianan757-gif.promptshelf`，应用标识固定，不随GitHub用户名变更。已有平面库保留 `library.db`、`assets/`、`cache/thumbs/`、`staging/`、`logs/`；整体恢复后的新库位于 `storage/<id>/`，由 `active.json`选择，`active.previous.json`保留上一库入口。

保留整个资料根，不单独移动数据库、原图或手工编辑指针。schema保持2，但旧D程序不认识E/F的active布局，不能直接用旧D程序操作恢复后的资料根；Git回退不等于资料库降级兼容。离线CLI与GUI共用库锁，同一资料根不能同时占用。日志`logs/poc.jsonl`可能含本机路径和窗口身份，外发前需脱敏。

## 已知限制与验证边界

- 安装包未签名；未发布稳定版或创建GitHub Release。源代码与文档已入Git，安装包、EXE、私人样本和完整本机日志仍仅保存在本地。
- Windows优先；缺少WebView2的安装路径、其他Windows/DPI环境、macOS、真实断电、实际超过4GiB的ZIP尚未完整验证，不能据已测格式结构推断通过。
- 粘贴进度表示已派发，不代表目标应用完成接收/上传；目标可能选择不同剪贴板格式或转码。已有用户工作流验收不等于所有应用版本的兼容性矩阵。
- 资源/响应是本机100案例/500合成图的描述性基线，无达标阈值，不承诺长期无泄漏。主库响应只有单次创建观察；中断/最小化可能影响资源差额，工作集相加可能重复共享页。CPU旧取整数据已标注无效，使用修正后采样。

## 成果与后续

[最终准备报告](release-preparation.md)、[补测证据](../validation/release-preparation/)、[A～F架构与阶段记录](../../Prompt%20Shelf%204.4%20正式开发架构与实施计划.md)保留供追溯；旧pending状态属于当时记录，本次批准已更新当前状态，未改写历史测试结果。

本次没有新增功能、重构或重复全量测试。最终收尾完成，后续仅等待用户发布指令；未经批准不创建Release、不发布正式稳定版、不自动上传任何本地构建产物。本地PROJECT.md沿用已有忽略规则，仅保存当前项目状态；公开状态以本说明和README为准。
