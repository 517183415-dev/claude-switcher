# R11 / F18 兼容整改计划

保留 CLI、IPC、DTO 与权限，避免破坏无法完全枚举的外部消费者。当前前端没有这两个准备 IPC 调用。CLI 仍消费 core readiness；不改 CLI。旧 journal 恢复代码保持。

1. 修改前先增加当前真实 KeychainMetadataProbe::run() 投影回归。覆盖 VerifiedPreserved、HandlingPlanAvailable、Unresolved、全部 backend blockers 与 ready=false。
2. bridge 将前两种兼容 assessment 视为已处理。ready 仍要求核心 ready 且无 blocker。备份接口继续明确不支持，backup_allowed 固定 false。兼容调用直接返回 completed=false 和无需迁移备份的指引，不做系统或资料访问。
3. 活动脚本等待开窗自动检查，初次应有1条，手动检查后2条；消除重复文本 strict locator，并更新日志。沿用已有 App VM 自动检查回归。
4. 使用离线定向 Rust 测试、既有 App VM 回归、Python/JS 语法及生产 Personal/Public 编译。真实浏览器、Tauri IPC 和账号保持 NOT_RUN。

写范围只有 bridge.rs、e03_core_preparation.rs、activity-refresh-regression.py 三文件。旧 in-place/account/repair/install journal 的实现、命令注册、权限和 DTO 均保持。测试使用合成数据与临时 HOME；不安装依赖，不使用 C2C，不运行产品网络、系统或真实账号。
