# B1 正式主库 UI 还原：交付与验收状态

日期：2026-10-25。范围仅为正式主库 UI；不实现案例保存业务、不进入 B2、不制作新 Demo。

**当前状态：2026-10-25 用户明确确认“B1 正式主库 UI 已通过用户视觉验收”。本轮仅整理提交与交付，不进入 B1 写入业务或 B2。** 自动化实测范围与未覆盖项保留如下，用户视觉验收不等于所有交互已自动化验证。

## 基线与实现范围

- 阶段 A 保存为独立提交 `b349f6136c85d0e3611a026679f956b86305668a`，标签 `baseline-stage-a`。B1 UI 为其后独立提交，通过 `dev/4.4-b1-ui` 开发分支交付；main 保持 4.3 发布基线。旧 exe 和测试库仅保留在本机。
- 主库从 V4.11 冻结 HTML 的布局、样式和 SVG 转为独立 React 组件，结合 V5.0 顶栏入口。旧 PoC 主库分支不再作为运行入口；保留已验收快捷面板及系统链路。
- 主库和快捷面板使用明确的组件/样式加载分支，修复发布产物只加载快捷面板样式的问题。仅复制原型视觉规则，不复制演示数据存储、重置和模拟中断工具。
- 本轮可操作的是浏览、搜索/筛选、布局/排序、侧栏和二级栏、参考图翻页、窗口控制。数据来自现有 SQLite；日期、来源、分类、快捷标签状态和回收站数量只读投影。
- 新建、编辑、收藏修改、分类管理写入、回收站和备份恢复等入口尚未开放，点击只提示，不写库、不显示假保存或草稿保护成功。V5.0 草稿及恢复完整交互留在相应业务阶段。

## 代码与证据

GitHub 审查入口：[公开验证摘要](../validation/b1-ui-results.json)。下列 `poc/reports/` 与 `evidence/` 原始日志、截图仅存本机，不随公开源码提交；公开摘要基于这些既有记录整理，本轮未重新执行功能测试。

| 位置 | 用途 |
|---|---|
| `poc/src/main.tsx` | 分离两个窗口的组件与 CSS 加载 |
| `poc/src/library/`、`poc/src/library.css` | 正式主库、卡片、导航、分类栏、原型图标与浏览逻辑 |
| `poc/src-tauri/src/data.rs`、`store.rs` | 只读 DTO 增补现有字段，无 schema/写入业务变更 |
| `poc/reports/b1-ui/` | 本轮测试、发布资源检查与构建记录 |
| `evidence/b1-ui-independent/review.md` | 独立静态复核，不能替代视觉验收 |
| `evidence/b1-ui/baseline.json`、`visual-fixture.json` | 回退基线及独立合成测试库说明 |

正式原型来源：`Prompt Shelf 4.2·v2.0/demos/01_V4.11_素材管理UI冻结基线.html`、`02_V5.0_数据保存回收站交互.html`。用户后续确认的快捷面板常驻、560 宽度和半透明圆形箭头继续有效。

## 已完成的验证

| 检查 | 结果及证据（位于 `poc/reports/b1-ui/`） |
|---|---|
| 前端逻辑 | 19 项通过，`frontend-tests.txt` |
| SQLite 只读 DTO | 1 项定向测试通过，`readonly-dto-test.txt`；本轮未重复全套 Rust/PoC 测试 |
| TypeScript / Vite | 通过，`frontend-build.txt` |
| 发布 CSS 分支 | 通过，`packaged-css.json`、`check-ui-build.txt`；检查实际产物中两个窗口的样式隔离，不等同于视觉验证 |
| Windows GUI / CLI 发布构建 | 均通过，`windows-release.txt`、`cli-release.txt`、`build.json` |
| 冻结源码复核 | 54 个文件与清单匹配；独立静态审查未发现阻断问题，见 `evidence/b1-ui-independent/review.md` |

新 GUI 为 `poc/src-tauri/target-stage-b1-ui/release/prompt-shelf-poc.exe`，SHA-256：`f4b7bdfc7d0983616df54d6c2900f27142b62019425ee68ab833d033000dcb8f`。阶段 A 与此前 round7 的 exe 哈希未变化。2026-10-25 20:23 已实际启动该 GUI，PID 17440，使用独立合成库。

已验证的 Windows 平台实现、队列、Esc 及快捷面板样式/图片轨道文件相对 A 基线无改动；`App.tsx` 移除了旧主库 JSX，保留快捷面板 JSX。本轮未重跑系统粘贴验收。

## Windows 视觉验收

用户本轮明确授权 Computer Use，主控通过 Windows 实际窗口操作与截图检查；没有调用系统粘贴或写入真实资料。阶段 A 旧只读测试进程已结束，其 exe、测试库均保留。

| 实际程序检查项 | 状态 |
|---|---|
| 普通窗口：顶栏、导航、独立搜索与筛选、卡片间距 | 1360×860 内容区实际显示正常；旧默认按钮/错位消失。截图 `01-library-normal.png` |
| 最大化与还原 | 实测通过；2560×1392 捕获画面显示三列，恢复普通窗口后重新排为一/两列。截图 `03-maximized.png` |
| 原生移动与缩窄窗口 | 自动化尝试未成功，未判定通过；尚不能区分工具与原生行为，需补充确认 |
| 项目/标签二级栏 | 展开/收起、项目单双列、项目1筛出15条通过；左右分隔条实际拖动通过。截图 `02-project-filter-double.png`、`06-secondary-resized.png`。二级栏搜索未单独实测 |
| 常规/瀑布流、空结果 | 切换通过；无匹配词显示0条及空结果提示，清除恢复100条。截图 `04-masonry-maximized.png`；空结果见本轮工具观察记录 |
| 参考图轨道 | 左右翻页实际生效，无原生图片滚动条；半透明圆形样式沿用已确认 CSS。截图 `05-reference-carousel.png`，部分鼠标悬浮位置被光标遮挡 |
| 快捷面板 | 实际呼出560宽面板，样式正常；打开主库后 Esc 隐藏快捷面板成功。此次未重测跨程序粘贴 |

截图目录：`evidence/b1-ui/windows-visual/`。启动与就绪日志：`evidence/b1-ui/manual-synthetic/logs/poc.jsonl`，与 `evidence/b1-ui/launch-20261025-202302-2072355.json` 共同记录测试版本与库。截图05/06包含其他应用悬浮窗的局部遮挡，不用于证明被遮挡区域无问题。

对照边界：冻结 V4.11 的 CSS/结构已逐项静态核对，Windows 渲染画面已实际检查；尝试在浏览器打开本地原型时，工具因 `file:` 协议安全策略拒绝，未绕过。因此没有完成“原型浏览器渲染图与程序图逐像素比较”，不能声称像素级完全一致。

原生拖动复核：开发方只读核对后未发现缺失初始化或权限；发布 JS 含拖动属性、主库具备 start-dragging 权限且默认可缩放。网页分隔条与 Win32 原生移动走不同输入链路，现有自动化结果不足以认定产品缺陷。因此没有为此增加权限或改写原生窗口实现；窄窗口仍是待验项。

合成测试库为 `evidence/b1-ui/manual-synthetic`：100 案例、500 图片，仅此隔离库预设 6 个快捷标签以覆盖 UI 展示；未修改正在运行的 A 库或生产导入规则。不同测试图片/文字不属于原型视觉差异。

启动入口（先从托盘退出旧版，不强制结束用户进程）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\scripts\start-b1-ui.ps1" -Fixture synthetic
```

启动后从托盘打开“主库浏览”。用户视觉验收已通过；本轮按要求仅提交交付，后续 B1 业务开发等待另行授权。
