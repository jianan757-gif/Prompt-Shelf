# 公开验证证据

本目录收录公开验证摘要。round5～round7 为历史证据副本；stage-a 为 4.4 本轮针对性验证。机器路径已脱敏；原始日志和私人截图留在本机。

| 文件 | 来源及结论 |
|---|---|
| `b1-ui-results.json` | 既有前端19项、DTO定向1项、构建与CSS检查；Windows主要界面实测和用户视觉验收，明确自动化未覆盖项。原始截图/日志仅留本机 |
| `stage-a-results.json` | Rust harness 47、前端 15 通过及两个 release 构建；100/500 离线导入/重开/拒绝重导通过。悬空链接分支因权限未实测，GUI 接入冒烟待人工验收 |
| `round5-independent-results.json` | 独立复核汇总；Rust 27 项、前端 15 项通过 |
| `round6-rust-tests.txt`、`round6-results.json` | 主控补测 Rust 32 项、前端 15 项及构建通过；独立验证仅完成局部静态审查 |
| `round7-frontend-tests.txt` | 前端 15 项通过；本轮 Rust 仅改版本标识，未重复全套 Rust 测试 |
| `synthetic-integrity.json` | 100 案例、500 独立原图样本完整性检查，不是性能报告 |
| `user-acceptance.md` | 用户反馈摘录，2026-10-25 确认测试通过并完成 4.3 |

历史报告中本机证据路径不一定随公开版上传。未提供完整逐应用图片接收矩阵、CPU/内存基线或统一条件下的 P95 性能统计。运行日志中的 completed 表示内部派发完成，不是目标应用上传或接收确认。
`round7-build.json` 为已有构建记录的公开副本，仅移除机器绝对路径；其中人工验收 pending 是构建当时状态，后来用户通过反馈见 `user-acceptance.md`。该记录对应原 round7 构建，不是本次 Git 提交的重新构建。
