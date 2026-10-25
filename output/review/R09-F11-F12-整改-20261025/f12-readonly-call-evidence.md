# R09/F12 本地只读调用与测试入口

状态：READ_ONLY。未执行 cargo、rustup、产品、GUI、服务、网络或账号命令。未使用 C2C、外部助手或浏览器。以下为当前源码证据与建议，非运行验收。

## 实际启动链

`main.rs:29 load_root_record → :33 source_local_pending → :34 RootLock::acquire → :35 recover_pending_repair → :37 xpc::serve`。
任何错误沿 `?` / `and_then` 到 `main.rs:39–41`，打印并退出1，尚未建立 XPC listener。
这里判断的是 SourceLocal transaction 文件是否存在，不是 InstallRecord 是否为 SourceLocal。正常 source-local 安装在该文件不存在时也走自动恢复。

## 合法业务冲突与损坏输入分界

- `platform.rs:837 recover_saved_plan` 先检查政策/时区和原始/目标 service 一致，再读当前状态。
- `:849 repair_recovery_conflict`：时区或 IPv6 已被外部改成 original/target 两者之外的值。保留 journal、不覆盖外部值是正确行为；服务不可达是 F12 问题。
- `:137 network_service_unknown`：原网络服务改名或删除。`timezone_unreadable` / `ipv6_unreadable` 是实际读值不可识别。不能把它们当“恢复成功”。
- `command:14–54` 可产生 `system_command_spawn/failed/output/output_large/encoding/timeout`。恢复读取、回写或复核时均可能触发，恢复失败应保留事务；未知错误不得整体放行。
- `root_operation.rs:145 root_transaction_busy` 与 `:212 root_installer_active` 是合法互斥/安装窗口。现有 XPC 每请求仍会核对 RootLock，因此服务存在不代表可执行写操作。
- `repair_recovery_verification`、completion 写入/清理失败也会退出。它们不能记为成功；应保留可诊断的事务状态。

必须 fail closed：

- `platform.rs:400 load_root_record`：父路径不是 root 所有不可写目录、文件不规则/非root/可写/过大、读取或JSON错误、`InstallRecord::validate`失败。
- `recover_pending_repair:807`：`repair_recovery_read/metadata/permissions/schema`。不能把损坏 JSON、链接或不可信文件当合法业务冲突。
- `recover_saved_plan`：`timezone_invalid/network_service_invalid/repair_recovery_service` 属于记录或参数非法，不能只按“恢复失败”放宽。
- `xpc.rs:184 serve`：root身份、record.validate、固定角色签名要求不变。连接 owner UID :208、系统签名gate :214、每消息authenticate :243 不变。
- `policy.rs:120–126` owner/role检查；:174–184 session绑定、sequence抗重放必须保持。
- 自动恢复入口的metadata校验目前未显式检查nlink；`read_repair_file:719`已有nlink==1检查。建议复用更严格读取，不以旧入口的较弱检查为新受限模式的可信判据。

## 现有 status / recover 能力

`policy.rs:286 RepairStatus` 仅 GUI/Installer角色，在已绑定session中调用 `SystemActions::repair_status`，无额外管理员授权。
`platform.rs:781 read_repair_status` 校验64位hex plan_id，读root journal与completion，核对owner，返回实际system state。
`status_response:737–779` 拒绝owner错配和pending缺completion，pending优先于terminal receipt；状态读取失败仍返回真实错误，不能伪造原值/目标值。
`policy.rs:245 AuthorizeRepair { recovery:true }` 要求pending状态和真实管理员external_form。
`policy.rs:274 RecoverRepair` 要求GUI/Installer及单次、30秒、绑定完整plan的recovery grant。
`platform.rs:185 recover_repair` 在恢复写入前先 `read_repair_status` 做durable owner绑定，再验证plan_id，调用同一恢复器。Guardian不能恢复或读取事务结果。

## 最小受限服务建议（最多3个生产文件）

1. `main.rs`：把可信启动恢复的业务冲突映射为“继续提供受限服务”，不吞全部Err。保留root record失败时拒绝服务。不要额外增加恢复器。
2. `platform.rs`：抽出可测试的只读可信pending判定/启动恢复结果分类，复用root journal与owner/receipt验证。只有可信pending的业务冲突进入受限状态；读取/metadata/schema/owner错误继续失败。状态查询遇到外部服务缺失时返回明确错误，保留journal。
3. `policy.rs`：经现有身份检查后，pending时只允许建立session、查询该恢复状态、获取recovery授权、显式RecoverRepair。禁止新preview/apply、普通授权、managed restore/commit及timezone reconcile。门禁必须放在expired ApplyRepair的`plan_not_started`分支之前（:129），因为该分支本身也会写completion。每请求重读可信pending，成功恢复清理后才恢复普通动作。

`xpc.rs`不需改协议/签名/RootLock；继续使用现有每请求锁。SourceLocal installer独占事务及其health allowlist保持原逻辑，不能把它转成普通repair recovery。
该方案需要新增小结果类型/门禁，不需要新RPC、额外修复器或第四个生产文件。RootLock busy只给出拒绝响应，不能绕过锁执行status或recovery。

## 现有测试接缝与应补行为

- `policy.rs:443 AuthorizationBoundary` 已记录授权消耗和mutations；:487 authorization_fixture / :430 request 可复用。
- 现有 `authorized_apply_is_single_use_and_recovery_requires_new_scoped_proof:595`、`recovery_plan_change_or_apply_scope_cannot_authorize_recovery:628`、`owner_role_session_sequence_and_expiry_matrix:833`、`guardian_cannot_request_root_recovery_or_read_transaction_results:982`、SourceLocal identity矩阵:1116/1145/1163 应保持。
- `platform.rs:489 status_requires_matching_plan_and_pending_overrides_completed_receipt` 已有只读SystemActions fake，适合补pending+外部状态错误，不触真实设置。
- `platform/dns_cache.rs:44 test_io` 是thread-local真实生产调用拦截；commands/journal/pending都可观察，未知命令panic；现有:146测试已经调用真实recover_pending_repair。新增启动分类测试应使用纯closure/fake而非真实main/launchd。
- 需要的独立回归：可信冲突仍进入serve回调；损坏记录不进入serve；pending时所有非恢复写动作零调用（含expired apply）；status不消耗管理员授权；recovery错误不清journal且可再次查询；fresh recovery grant成功后解除门禁；SourceLocal pending不触自动恢复；root lock忙不触写操作。

## 源码快照哈希（只读采集）
- `crates/switcher-helper/src/main.rs`: `a2a5d6de73f7fae8aa4cf1c1c6fdf2f18e658cbea2cc2340b496842803c886d5`
- `crates/switcher-helper/src/platform.rs`: `766df55f8815434d8a935cbb780b2050766a636ecdd249c7e543fbcae0882197`
- `crates/switcher-helper/src/policy.rs`: `b28a68ddbf2201004ce5fa047da7bbbf4515cb6976a5b74336f2ce10a2aa377c`
- `crates/switcher-helper/src/xpc.rs`: `778698407e07be6312f0299f8cd1b790e63ed42776f01e011fd64787f44cb833`
- `crates/switcher-helper/src/root_operation.rs`: `4e87fffdb74eca123cd34e724fad64d1f2ed9b90c695d0c5c61087b9b6603825`
