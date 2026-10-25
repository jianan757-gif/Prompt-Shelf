# Prompt Shelf｜4.2 Tauri 单主线任务书 v2.0

## 决议及范围

**正式技术路线：Tauri 2 + Rust + React/TypeScript + SQLite + 本地 assets/ 原图。** 产品 **Windows 优先，后续 macOS**。WPF 因 UI 重写和平台独占成本被淘汰；GPUI 暂缓；PoC 不做双方案对比，只进行 Tauri Windows 实机粘贴、焦点和资源准入。

## 本包文件

- `PromptShelf_4.2_Tauri单主线技术选型与PoC任务书_v2.0.docx`：完整 Word 文档（便于评审批注）。
- `PromptShelf_4.2_Tauri单主线技术选型与PoC任务书_v2.0.md`：同内容 Markdown（便于 Codex 读取）。
- `Demo_入口.html`：三份浏览器 UI 原型的本地入口。
- `demos/01_V4.11_素材管理UI冻结基线.html`：主素材管理 UI，**冻结，不改动原文件**。
- `demos/02_V5.0_数据保存回收站交互.html`：草稿、回收站、备份交互样机；其中 Base64 JSON 备份、旧 URL 校验不是正式实现。
- `demos/03_快捷调用面板早期样机.html`：单列、44×90 队列按钮、Esc、快捷键与模拟输入区。
- `previews/`：对应产品视觉截图，用于人工评审。

## 技术与原型区别

三份 HTML **均不是原生 Windows PoC**，只证明网页中视觉/交互样机能运行。本文要求的全局快捷键、跨程序焦点恢复、多格式粘贴、队列和 WebView2 资源回收需要 Codex 实际编译 Windows 程序，并提供真实测试证据。**不得把网页日志当真实粘贴成功。**

**正式数据差异**：生产完整备份使用 ZIP（元数据/SQLite 一致性快照、原始图片、校验清单），用户纯域名链接自动补 `https://`；不可复制 V5.0 旧 JSON/URL 实现。

## 推荐复核顺序

阅读文档 §0/§1 了解平台决议 → 查看 V4.11 视觉基线 → 体验 V5.0 草稿回收站 → 查看快捷面板 44×90 按钮及 Esc → 核对 §4 的平台架构与 §6 的 PoC 通过门槛 → 最后交付 Codex 执行 §9。
