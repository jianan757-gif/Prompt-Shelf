# B2 验证记录

2026-10-27；所有自动化使用临时或独立测试目录。用户 B1 库未迁移，未开展 Windows GUI 自动操作。

| 项目 | 方法 | 结论与证据 |
|---|---|---|
| 自动暂存时机 | Node 模拟时钟：停输600ms、连续2s、图片立即刷新、结束停止定时 | 通过，`frontend-tests.txt`；不等于实际磁盘延迟指标 |
| 草稿持久化与分类隔离 | 真实 SQLite：不完整字段、已知分类外键、待登记名称 | 通过，Rust 草稿测试 |
| 原子提交/放弃失败 | SQLite 触发器故障注入，验证正式记录、草稿及会话保留 | 通过，Rust 草稿测试 |
| 新建失败、共享引用和重排 | 真实临时文件、SQLite、原图哈希 | 通过，`new-draft-images-test.txt` |
| 迟到写入 | 真实工作线程与通道控制，丢弃/保存/窗口会话结束后放行写入 | 通过，`late-write-test.txt` |
| 迁移与旧版拒绝 | v1→v2事务、迁移故障回滚；本地 B1 CLI 拒读 v2 且数据库字节未变 | 通过，草稿测试和 `rust-integration.txt` |
| 异常退出恢复 | 自生成测试子进程写草稿后强杀，再重新打开 SQLite；已有/新建草稿、共享图片 | 通过，`rust-integration.txt`；强杀的是测试工作进程，不是 GUI |
| 关闭协议 | pending gate、epoch失效、finishing认领、拒绝取消、失败可重试 | 通过，`closing-tests-final.txt`（4项）；是状态协议测试，不是实际 Alt+F4 |
| 正式 UI 构建 | TypeScript/Vite；检查实际打包 CSS 隔离及冻结原型样式 | 通过，`frontend-build-close-fix.txt`、`check-ui-build-final.txt`；视觉验收未运行 |
| Windows 界面与粘贴 | 用户操作关闭/切换/取消/恢复及保存后快捷粘贴 | **待验收**，清单见 [交付记录](../../public/b2-edit-safety-delivery.md) |

GUI/CLI 最终 Windows release 构建通过，证据为 `windows-release-final.txt`、`cli-release-final.txt`；源码清单与历史程序哈希核对通过。程序 SHA-256、测试批次及未验证边界见 [build.json](build.json)。启动脚本 PowerShell 语法检查通过，未启动 GUI。

## 独立复核

独立验证窗口只读复核关闭协议、草稿事务和前端路由，最终确认此前指出的代码阻断项已处理：关闭执行/取消并发、失败解除请求、消息发送失败解除请求、搜索值捕获及恢复弹窗初始焦点。本轮独立复核未重跑测试、未操作 GUI；不能替代人工实机验收。完整本机记录为 `evidence/b2-independent/review.md`。

## r2 补修

首轮模拟时钟漏掉了浏览器原生调用错误，自动草稿的实际运行结论已由 [r2 定向验证](../b2-r2/README.md)补充。保留本页历史证据，不将首轮程序解释为已通过完整自动草稿验收。

## 日志边界

- 本机原始过程日志：`poc/reports/b2/`；独立只读审查：`evidence/b2-independent/review.md`。这些本地目录不公开。
- 本目录的日志副本只替换本机绝对路径，测试结果不改写。构建产物、数据库、截图和用户图片不提交。
- 早期 `rust-lib.txt` 存在编译类型错误，已修复；公开保留的 `rust-prior-run-with-fixture-failure.txt` 中首次强杀测试失败来自测试夹具复用图片引用 ID，修复为独立引用后，`rust-integration.txt` 已通过。早期失败保留在本机，不能将旧日志当作最终通过。
- 原 Windows 粘贴、焦点、队列及 Esc 源码未修改；B1 已验收记录继续有效，但 B2 保存后实际粘贴仍待定向人工回归。
- 没有把重复执行、lib/main 重复编译的测试或子进程入口累计成独立测试数量。

## 复现

```powershell
cd poc
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'src-tauri\target-stage-b2'
pnpm test
pnpm build
node scripts/check-ui-build.mjs b2
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

异常进程测试仅结束自己创建的工作进程。旧 B1 CLI 拒读测试依赖本地保留的 `target-stage-b1-business/release/prompt-shelf-store.exe`；不存在时会跳过该子检查，不能将其他机器的测试通过解释为已验证旧程序拒读。
