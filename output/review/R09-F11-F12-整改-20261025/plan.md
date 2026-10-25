# R09 本地最小整改与回归计划

状态：EXECUTING_LOCAL。用户已停止 codex-with-chatgpt；不使用其 CLI/MCP、ChatGPT 页面或外部助手。

## 文件所有权

- root：platform.rs、main.rs；F11 三个固定临时写点、F12 可信 pending 读取与启动恢复分类。
- f01_recovery_tests：policy.rs 及同模块隔离回归；不得写 platform.rs/main.rs。
- f01_review：只读独立审查；不改生产文件。

## 最小变更

1. F11：三个写点复用本模块已有随机 token 模式，使用同目录独立临时文件。保留文件 sync、目录 sync、create_new、NOFOLLOW 和现有 RootLock。旧固定残留不清除。写前后核对临时文件身份，外部替换不发布、不删除。已存在最终 repair journal 必须保留。只抽出三个生产写点需要的磁盘接缝，不改 RPC/安装合同。
2. F12：从严格只读 journal/receipt 校验获得 pending ID。坏类型、权限、链接、schema、owner/plan 绑定继续拒绝。服务启动仅把可信 pending 的明确业务冲突转为受限服务；未知错误继续拒绝。正常自动恢复沿原恢复器执行。SourceLocal 安装事务独占路径不变。
3. policy：身份检查后、expired ApplyRepair 的任何写入前检查 pending。允许现有会话、只读状态/guardian 诊断、匹配 plan 的 status、recovery 授权及 recover；阻止普通授权、preview/apply、安装提交、系统恢复、timezone reconcile、health receipt 写入。恢复成功清 journal 后普通请求才重新可用。签名、owner、role、sequence、单次授权、RootLock 不放宽。

## 回归和停止条件

- 先运行可复现旧行为的 RED，再最小修复。F11 覆盖创建后、部分写后、rename 前中断，旧残留/符号链接/外部替换留存，最终 journal 不覆盖。
- F12 覆盖可信冲突不退出监听流程、坏记录仍拒绝、普通 pending 恢复、SourceLocal pending 不执行普通恢复；policy 覆盖全动作矩阵、expired apply 零写入、角色/owner/session/sequence 不放宽、显式 recovery 新授权及门禁解除。
- 所有测试仅合成临时目录、fake SystemActions 或线程局部拦截。Rust 使用已安装直接 cargo/rustc/rustdoc、固定缓存及 --offline --locked，不调用 rustup 代理，不下载。
- 定向回归后运行 helper 现有受影响测试、clippy、fmt、diff。逐条保存 argv、退出码、日志、测试数和哈希；独立只读审查确认后更新进度。
- 真实 helper/XPC/GUI/服务/系统设置/账号和产品网络均 NOT_RUN。无需安装。R08 已发生的历史工具链下载异常保留，不以本轮通过覆盖。

流程：

    freeze -> RED -> minimal fix -> isolated GREEN -> local review
