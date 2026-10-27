# B1 业务交付记录

2026-10-27：用户完成 Windows 实机验收，确认测试清单 1～8 全部通过，并确认搜索框修订复测通过。共享图片已通过用户补测；早先的 `case_title_required` 为未填写标题触发的校验，不是共享图片故障。

## 实际完成

- 案例新建、原卡片编辑、手动保存与取消；标题、正文、项目、标签、来源和图片引用通过 SQLite 事务保存。
- 文件选择、拖放及 Ctrl+V 图片导入；效果图/参考图替换、引用移除与排序；复用 SHA-256 原图存储及缩略图授权。
- 保存后主库与快捷面板重读正式数据；快捷调用校验所显示案例的版本，避免旧图片下标解析新顺序。
- 复用原有 Windows 粘贴、焦点恢复、队列、Esc 和窗口机制。

## 验证结果

| 验证 | 结果与证据 |
|---|---|
| 新建保存、修改/取消、图片管理、文件/拖放/剪贴板导入 | 用户实测通过，2026-10-27 本聊天反馈 |
| 共享图片、双窗同步与快捷文本粘贴、重启保留、编辑界面 | 用户实测通过，2026-10-27 本聊天反馈；不等同于所有目标应用兼容性验收 |
| SQLite 事务回滚、版本冲突、共享图隔离、会话失效 | 既有 Rust 自动化通过；本地 `poc/reports/b1-business/rust-tests.txt`，lib/bin 存在重复执行，不累计为独立场景数 |
| 共享图片定向复核 | 通过；`poc/reports/b1-business/shared-image-recheck.txt`，覆盖引用重排/替换不改变其他案例及原图哈希 |
| 前端逻辑 | 25 项通过；`poc/reports/b1-business/frontend-tests.txt` |
| 独立审查 | 未发现确定阻断缺陷；本地 `evidence/b1-business-independent/review.md`，属于静态检查 |

## 验收后的小修订

按用户要求移除主库、项目/标签搜索和快捷面板搜索聚焦时的蓝色描边，保留普通边界；编辑器标题/正文为空时显示中文必填提示，后端约束保持不变。用户已确认搜索框复测通过。构建及发布 CSS 静态检查通过，详见[公开证据](../validation/b1-business/README.md)；其中区分自动化、独立静态审查和用户实机结果。

## 关键代码与复现

- `poc/src/library/CaseEditor.tsx`、`useCaseEditor.ts`：原位编辑、导入、保存与取消。
- `poc/src-tauri/src/cases.rs`：版本校验、单事务保存和内存编辑会话；`editing.rs`：受限主库命令；`clipboard_input.rs`：图片剪贴板读取。
- `poc/src/refresh.ts`、`App.tsx`、`library/LibraryManager.tsx`：双窗刷新及迟到响应隔离。`platform/`、`queue.rs`、`escape.rs` 保留原实现。
- 新增官方 `tauri-plugin-dialog`，现有 image 增加 BMP 解码；没有新增 ORM、连接池或独立权限系统。

从仓库根目录执行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-b1-business.ps1 -Fixture empty`。须先按根 README 构建 GUI 和 CLI；默认测试库为 `evidence/b1-business/manual-empty`，再次启动继续读取该库，`empty` 不会清空数据。私人案例和原图不随仓库发布。

## 范围与下一步

B1 业务实机清单已通过。B2 自动草稿、异常恢复、完整未保存关闭拦截尚未实现；关闭窗口可能丢失未保存输入。取消导入可能保留未引用的完整资产，清理留待后续阶段。原图预览、完整分类管理、回收站和备份未开放。

本交付位于 `dev/4.4-b1-ui`，以 B1 业务独立提交接续原有历史：阶段 A `b349f61` → B1 UI `165d236` → UI 回归验收 `c8c469b` → B1 业务交付。main/v0.1.0 保持原 4.3 基线，不合并、不另发正式版本。B1 完成后暂停，等待审核，不进入 B2。
