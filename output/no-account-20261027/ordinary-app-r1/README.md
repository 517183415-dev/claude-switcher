# 普通 App 有限业务验证

源码提交：`9d6a3efcfc6e907ae7797633a843720533a94b34`。范围以 `receipt.json` 为准。此目录没有产品修复，不把 UI 自动化观察扩大为全部 IPC/ACL 验收。

`runtime.json` 与 `archived-source-inputs.json` 记录精确归档及构建输入。`bundle-*.json`、`artifact-*.json` 与对应 stdout/stderr 保存两版构建、22 项静态检查、复制和 ad-hoc 签名的原始结果。构建回执中的 `product_launch: NOT_RUN` 是构建时点状态，随后实际运行以 `receipt.json` 和 `runtime-*/r*.result.json` 为准。

`launch_app.py` 启动正常 App 入口，只使用已验证的 Seatbelt profile 和合成 HOME。UI 操作经 CUA 执行；文本与 PNG 是观察证据。完整重启文本见 `personal-draft-after-restart.txt`、`public-after-restart.txt`；部分其他文本是 AX 差异或无变化提示，不能独立作为完整状态证明。进程收到测试停止信号，因此结果为 -15；不能据此判定产品崩溃。Personal r3、Public r1 在用户请求设计预览时中断，没有作为额外用例计数。

`personal-before-restart.json`、`public-before-restart.json` 保存独立磁盘检查点。`personal-before-public.json` 用于核对 Public 验证期间 Personal 文件不变。`finalize_evidence.py` 核对这些记录、最终合成文件、版本与哈希，再产生 `receipt.json` 及 `evidence-manifest.json`；它不重新执行产品测试。

Git 只保存有界证据、脚本与 profile。普通 App 二进制、完整源码归档、前端 dist 和合成 HOME 留在本机，不纳入 Git。复跑构建需要本机已有工具链和离线缓存，路径见 `runtime.json`；不能声称从证据目录单独即可离线冷启动复现。直接 securityd 连接负控与系统托管 WebKit 回收仍未证明。

后续 UI 任务只完成排期。设计参考和“保留当前布局”的约束见 `docs/design/SwiftUI流体玻璃重写-排期与设计约束-2026-10-27.md`。
