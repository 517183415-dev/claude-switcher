# R4 Rust 实施与验收状态

> 2026-10-24 UTC 旧版核心恢复最新状态：**统一安装已 committed，只读核对 components_verified。** root 服务属组错误已修复，journal/barrier 已清除，原组件备份保留。已安装 20261023 候选的四个二进制匹配冻结工件。真实 Personal 开窗自动完成 14 项检查，首次为 12 通过／2 提示／0 阻止；从当前 A 启动后，Claude 登录窗口出现，切换器自动退出。用户说明旧账号已封禁；参考会话「卸载并清理Claude Code」确认当时清理 CLI、保留 Desktop，不能把登录页归因于本次重写，也不标真实登录通过。未创建真实 B，A→B→A 仍未运行。随后时区变回 Shanghai，CLI 与两版新候选窗口均为 12 通过／1 提示／1 时区阻止，保留旧版每小时纠正周期。
>
> 原生 A 建档发现当前目录被替换，与旧版原位行为不符。**后续修复仅改 accounts/legacy.rs，25 项定向测试、strict Clippy、格式和差异检查及独立窄审通过。Personal/Public 新候选已构建、严格签名并验证真实窗口和只读 IPC，尚未安装。** 本轮不再请求管理员安装。已安装候选与新候选各自绑定证据。见[恢复结果](../acceptance/旧版核心恢复结果-2026-10-23.md)、[证据汇总](../acceptance/evidence/legacy-restoration-20261023/result.json)与[原位补丁](../acceptance/evidence/legacy-restoration-20261023/first-a-in-place/independent-review.json)。以下历史段落不作为本轮功能门槛。

> 2026-10-23 21:11 UTC 最新本机状态：**Personal r17 已管理员替换 committed。** 已安装 App/helper/guardian SHA 与冻结工件逐一一致。真实迁移备份及独立隔离恢复已验证，schema 3 收据已写入，backup journal 已清除；原始活动目录身份与旧备份保留。见[安装核对](../acceptance/evidence/e07/legacy-r17-installed-verify.json)、[真实备份](../acceptance/evidence/e07/legacy-r17-native-backup-verified.json)和[窗口结果](../acceptance/evidence/e07/legacy-r17-native-backup-result-ax.txt)。

> 21:24 UTC 已证源绑定失败来自 GUI 周期刷新更新共享 clock.json 的 previous。两个备份 root 的严格 hash 均通过，收据不变；只在内存替换该时钟 payload 即恢复原 source_digest。r18 已修复两个读取入口的正常采样持久化副作用，保留首次／换 boot／异常与中断恢复记录，迁移校验不放宽。30 项唯一精确测试、43 条命令通过，Astra scoped CLEAR；workspace/Public strict Clippy、fmt 和 diff 检查通过。见[时钟复核](../acceptance/evidence/e07/legacy-r18-passive-clock/independent-review.json)。见[脱敏诊断](../acceptance/evidence/e07/legacy-r17-clock-binding-diagnosis/root-integration-decision.json)。A 初始化、实际登录和重启仍 NOT_RUN；正常初始化保留活动目录原 inode，完整 F-08 恢复仍未完成，F-09 仍 WAIT_B。新检查为 12 通过／1 提示／1 时区阻止。只读日志确认 macOS timed 根据 Location 在 r17 安装前将时区改回 Shanghai；守护锁忙／待提交短重试已通过 10 项定向测试与 Astra R2 scoped CLEAR，已纳入 r18 候选，尚未原生验证。见[复核](../acceptance/evidence/e07/legacy-r18-timezone-retry-r2/independent-review-r2.json)。广告后置，无障碍专项按用户指令跳过；37 项完整验收状态不升级。下方 r16 及等待安装的段落为历史。

> 2026-10-23 21:38 UTC 候选状态：**Personal r18 已构建并冻结，尚未安装。** 主程序 SHA `7d1020dea700250c848fd548d0cad965915135c44724c7b8b6fd8aca059566b3`。App/helper/Guardian 严格签名检查、版本入口和公开安装预览通过；180 项输入无构建后漂移，相对 r17 仅时钟／两个读取 runtime／测试及 timezone_guard 共 5 文件变化。已正常退出 r17 空闲窗口。用户反馈安装过于频繁；r18 命令先不执行，先集中无需管理员的源级工作，再统一安排真机安装。真实账号初始化、启动、交换及 r18 备份准备状态仍未验证。见[冻结工件](../acceptance/evidence/e01/legacy-r18-release-staging.json)与[安装准备](../acceptance/evidence/e07/legacy-r18-preinstall-final.json)。

交接 ID：CS-RUST-20261022-R4。接收会话：01a161e7-0f87-7740-9f41-f36cf541df9a。
现行规格仅为 PRD v1.2 和工程执行文档 v1.2。
本记录保留实施历史，并标记当前交接状态。适用功能与交付条件以[37 项验收记录](../acceptance/验收记录.md)为准。当前局部原生 PASS 不代表完整验收项 PASS。

## 当前交接状态

2026-10-23 20:38 UTC：**Personal r16 本机管理员替换 committed。** 已安装 App/helper/Guardian SHA 均匹配冻结工件，真实窗口重启成功。真实检查首次因时区漂移为 12 通过／1 提示／1 阻止；应用地区修复后为 **13 通过／1 提示／0 阻止**，修复与安装 pending 已清除。自动纠偏的触发和漂移来源尚未证明。见[本机记录](../acceptance/evidence/e07/legacy-r16-native-install.json)、[复检窗口](../acceptance/evidence/e07/legacy-r16-native-checks-after-repair-ax.txt)。

真实迁移备份已执行，首副本复制完成后校验失败，暂存和 backup journal 保留，未交换账号。只读逐项比较 25,110 项、读取约 26.6 GB，内容 SHA、ACL、owner、mode、大小和 mtime 均相同；仅两个副本文件新增 com.apple.provenance，三次回读稳定。正在修复首次复制等价与后续严格工件绑定；不全局忽略系统属性。见[脱敏诊断](../acceptance/evidence/e07/legacy-r16-backup-diagnostics/comparison.json)。初始化、真实 A 恢复与登录仍未执行，B 仍 WAIT_B。CLI 构建入口和文档路径已补齐，本机 release 编译及 help 通过，尚不等于全部日常命令原生验收。见[CLI 交付检查](../acceptance/evidence/e07/legacy-cli-source-delivery.json)。以下等待 r16 安装的段落均为历史；37 项完整验收状态不升级。

2026-10-23 20:22 UTC：**Personal r15 已管理员安装 committed。** 主程序 SHA `0a3d7780…58c63` 与冻结工件一致；helper/Guardian 固定路径和身份通过，安装与修复 pending 均已清除，系统原值及成功回执保留。真实窗口完成 14 项检查：**13 通过、1 提示、0 阻止**；提示为首次使用尚无历史出口地址。见[本机安装](../acceptance/evidence/e07/legacy-r15-native-install.json)、[完整窗口证据](../acceptance/evidence/e07/legacy-r15-native-checks-ax.txt)。以下等待 r15 的段落均为历史。

真实账号备份在目录校验阶段被阻止，尚未复制账号资料。原因是构造器将旧版安全 `accounts/0755` 误按新私有目录检查。最小修复仅接受既存安全旧容器，保留其权限；新容器仍为 0700，`v2/transactions` 继续严格私有。5 项新增、1 项既有定向测试通过，Astra 独立增量复核为 scoped CLEAR；另 2 项既有复制/迁移测试、8 次 KB 级实际 ditto 备份与隔离恢复通过。见[诊断](../acceptance/evidence/e07/legacy-r15-backup-legacy-mode-diagnosis.json)、[复核](../acceptance/evidence/e07/legacy-archive-mode-review.json)。Workspace/Public Clippy、fmt、diff 检查通过。Personal r16 已构建，主 SHA `cb231eea…114ce`；严格签名、版本入口和只读安装计划通过，等待用户执行管理员命令。180 项构建源码与快照一致；相对 r15 仅 accounts.rs 改变。

空间预检发现 active 逻辑数据约 12.4 GiB、已分配约 11.5 GiB；现有备份、隔离恢复和初始化需要三个副本。约 30 GiB 可用空间不足以完成全部步骤。项目 debug 编译缓存已用限定 Cargo dev-profile 清理；dry-run 的所有路径均在本项目 target/debug，180 源文件与 6 个冻结二进制检查不变。清理后独立 df 回读可用约 318 GiB，足够本次复制；全卷空间变化不全部归因于本次缓存清理。源码、历史 release、证据及真实账号资料保留。见[缓存清理证据](../acceptance/evidence/e07/legacy-r16-cache-clean.json)。见[空间元数据](../acceptance/evidence/e07/legacy-r15-account-space-metadata.json)、[复制边界元数据](../acceptance/evidence/e07/legacy-r15-copy-boundary-metadata.json)。真实备份、初始化、A 登录继续 NOT_RUN，B 仍 WAIT_B；不升级完整验收项。

2026-10-23 20:01 UTC 安装器交接修复已冻结：仅同 owner／edition、已审阅的 helper 0.2.0 SourceLocal 显式替换可保留并交接已提交系统修复记录。严格核对原值、成功回执、安装身份和私有目录身份／权限；在提交、恢复和清理前复验，冲突保留事务。61 项隔离测试通过；Astra 增量复核为限定 CLEAR，独立重跑 8 项通过，首尾 SHA 一致。初次复核发现的目录权限 P2 已修复并保留历史。公开 plan 与保存的 plan／digest 一致，命令 Bash 语法通过。r14 主程序 SHA 仍匹配，root 修复和安装 journal 均不存在。真实 root 记录内容当前用户不可读；本机替换尚未执行，等待用户重试同一 r15 安装命令。见[最终复核](../acceptance/evidence/e07/legacy-r15-managed-state-review-final.json)与[集成检查](../acceptance/evidence/e07/legacy-r15-managed-state-integration.json)。本轮未升级任何完整验收项。

2026-10-23 UTC 当前安装阻断：正确摘要已到达事务入口，随后因长期 `system-original.json` 被无差别视为待恢复状态而拒绝。真实 root 修复 journal 不存在，原值文件仍为 root 所有、0600、单链接；r14 安装未改变。正在修复同 owner／同 edition／SourceLocal 显式替换的已提交记录交接，不要求先回退用户当前系统设置。未知记录、未完成修复和卸载继续阻断；不会删除原值或回执。旧实现新增成功交接回归已失败并命中该根因。见[现场诊断](../acceptance/evidence/e07/legacy-r15-system-records-diagnosis.json)和[冻结 helper 来源兼容证据](../acceptance/evidence/e07/legacy-r15-managed-state-compatibility.json)。在修复及复核完成前，不再要求用户重试。

2026-10-23 19:46 UTC r15 安装准备纠正：首次管理员替换返回 `plan digest changed`。原因是本地准备脚本错误地生成了 operation=replace 的摘要；安装器要求先提交公开 operation=install 预览的摘要，再在内部授权替换。当前已改为直接读取 `local_install.py plan` 的返回值。4 项限定检查通过：真实 main 的模拟替换入口接受公开预览摘要、拒绝旧摘要且未调用写入、重新预览一致、命令摘要与 Bash 语法正确。生产安装器、冻结 r15 工件和当前 r14 安装均未改变。失败记录与原计划保留，见[纠正检查](../acceptance/evidence/e07/legacy-r15-digest-correction-checks.json)和[失败归档](../acceptance/evidence/e07/legacy-r15-digest-failure-archive.json)。当前等待用户重试同一安装命令。以下“只读 replace 计划通过”仅表示当时生成成功，不表示旧摘要可被管理员入口接受。

2026-10-23 UTC r15 源码检查点：**账号迁移心跳竞争与 Surge 失败意图保留已修复，并完成限定独立复核。** 迁移实现 12 项定向测试通过，审查者重跑其中 7 项通过；地区 42 项、代理 11 项通过，增量审查者重跑其中 2 项通过，不重复计数。workspace/Public strict Clippy、fmt、diff check 全通过。Personal r15 已构建，180 源文件与构建前快照一致，主程序 SHA `0a3d7780…58c63`；App/helper/guardian 严格签名、版本入口、只读 **replace** 计划通过。当前仅等待用户执行 r15 管理员替换；r14 仍为当前安装。真实备份、账号初始化与 A 登录尚未执行。见[迁移复核](../acceptance/evidence/e07/migration-heartbeat-independent-review.json)、[代理复核](../acceptance/evidence/proxy-runtime/surge-app-open-review-final.json)、[集成检查](../acceptance/evidence/e07/legacy-r15-integration-checks.json)和[构建快照](../acceptance/evidence/e01/legacy-r15-release-staging.json)。本次仅 6 个生产源文件相对 r14 改变；旧版脚本、用户账号资料和既有广告工件保留。

2026-10-23 UTC 最新检查点：**Personal r14 已管理员替换 committed，真实系统修复完成。** 已安装主程序 SHA `cf41c23e…70b8e` 匹配冻结工件。真实窗口复检为 **13 通过、1 提示、0 阻止**；提示仅为尚无历史出口地址可比较。US／America/Los_Angeles／en_US 已保存，Rust Guardian 新鲜心跳、签名与身份通过，两个修复 journal 均已清除。首次修复后立即检查曾有守护待确认，复查后通过；原值保留。见[本机证据](../acceptance/evidence/e06/legacy-r14-native-core.json)。账号写入和 Claude 启动本检查点未执行；源代码中新补的 Surge 打开与迁移心跳修复尚未包含于 r14。旧版核心的下一步为完成稳定备份、原位初始化及真实 A 登录验证；B、其他代理和长期观察分别保留等待或未运行。以下等待 r14 的段落属于此前历史。

2026-10-24 用户明确调整实施顺序：“先实现旧版已经有的功能，再慢慢加”。当前阶段以旧版行为为基线：检查与启动、账号保存／切换／恢复、守护、修复／地区设置／代理诊断、更新与安装卸载。新产品扩展后置，已有代码保留。当前只收尾让旧功能可运行的 Personal 本地安装与旧服务交接，不继续扩展安装协议。两版 r10 已完成本机构建并保存；374 项产物断言、3 项 Mach-O 解析测试及 workspace/Public strict Clippy、fmt、diff 检查通过。r10 尚无真实窗口或管理员安装验收；本轮旧功能补齐改动不属于 r10。

旧版对照已补齐时区漂移纠正、DNS 刷新、Clash 置顶规则、守护缺失恢复和按实际出口设置地区。这五项已通过限定隔离测试及复审，未执行真实系统写入。Personal r12 已构建并归档，179 个源文件匹配构建快照；严格签名、只读安装计划、workspace/Public Clippy、fmt 和 diff 检查通过。命令行修复已接入固定已安装 GUI 身份，隔离测试与独立复审通过。r12 四项真实工件参数检查通过；真实窗口的修复预览、禁用执行和取消已观察。首次管理员安装因安装脚本恢复用户组的 Darwin 限制退出，尚未创建新应用或安装记录。脚本现已改用 libc 原始内核组表，并覆盖降权中途失败的恢复；45 项测试及独立复审通过，第二次真实安装已 committed，已安装组件和两项新服务核验通过，旧服务已退出；真实窗口读取系统原值并返回 11 通过、1 提示、2 阻止。首次地区配置尚未落盘导致守护 degraded 的源码问题已修复，系统修复未执行。受限 Clash 用户进程重启事务已实现，57 项定向测试及独立复审通过；root 常驻核心重载、实际规则回读及旧版 Shadowrocket／ClashX／Surge 的打开 App 行为仍待完成；sing-box、Auto／0dcloud 旧版本身需要手动启动，详见[旧版代理对照](../acceptance/evidence/proxy-runtime/legacy-proxy-parity.md)。r13 已完成 180 源文件快照构建、严格签名和两发行集成检查，并在本机替换 committed；[新组件及服务核验](../acceptance/evidence/e06/legacy-r13-installed-verify.json)通过。真实修复在偏好备份验证处中断：macOS 为复制品新增 com.apple.provenance，严格属性比较拒绝备份；未进入系统写入。原生恢复已完成，region/user journal 均已清除；[只读 CLI 预览](../acceptance/evidence/e06/legacy-r13-installed-cli-preview.json)通过且状态未改变。r13 真实窗口复查仍为 11 通过、1 提示、2 阻止（时区不一致与守护策略未完成）。按本机架构重新构建的 CLI 已通过 fix 与 set-region us 两项只读转发，详见[CLI 证据](../acceptance/evidence/e06/legacy-r13-fresh-cli-forwarding.json)；旧 target/release 产物的失败记录继续保留。该备份缺陷已完成源码修复：首次跨 inode 复制只允许新增 provenance，并保存完整备份属性用于后续严格校验。独立复核发现的恢复收据问题也已修复；25 项偏好修复测试、11 项地区测试及真实源只读复制／隔离恢复通过。见[独立复核](../acceptance/evidence/e06/r14-preference-copy-review.md)。r14 已完成 180 源文件快照构建、App/helper/guardian 严格签名、版本入口和只读替换计划，正在等待用户执行管理员替换。最新根集成复跑为偏好修复 25/25、地区 36/36、workspace/Public strict Clippy、fmt、diff 全通过；见[集成记录](../acceptance/evidence/e06/legacy-r14-integration-checks.json)和[构建快照](../acceptance/evidence/e01/legacy-r14-release-staging.json)。不将 r13 记为真实修复通过。Personal 仍按现行规格手动更新；Public 自动更新的源码交付路径后续处理。账号核心已有实现和隔离验证，真实登录恢复与 A→B→A 仍待验收。

当前优先级为“先把项目系统本身做完，再考虑补广告”。2026-10-23 用户明确选择 GitHub 源码及本地构建／安装脚本，不要求商业签名二进制发布。PRD v1.2 与工程 v1.2 已同步：商业证书、公证不再阻塞 SourceLocal 本机安装与源码交付；DeveloperId 更新分支保留。广告后置，无障碍专项继续跳过。

源码本地安装正在实施：schema2 root 登记 generation 和 GUI/guardian/helper CDHash，双端 XPC 身份校验保留；本地升级重新构建并走完整管理员安装。安装脚本已覆盖组件事务、健康提交和旧服务恢复；45 项隔离测试及 Personal 实际工件只读计划通过。独立复审发现的重复恢复、Applications 权限和旧 plist GID 问题已修复；最新签名解析修复已通过增量复审。第二次管理员安装已提交，真实安装健康和旧服务退出已验证；功能验收仍需逐项完成。决定见[源码本地安装补充](../plans/源码本地安装补充-2026-10-23.md)。

r9 是此前有真实窗口证据的历史候选，其证据不覆盖 r10、r11、r12 或本次源码安装改动。Personal r12 安装及 r13 替换已完成管理员提交、组件身份和 XPC 健康核验。真实系统修复、真实 A 登录恢复、A→B→A、完整动态隐私及长时观察仍未完成，系统核心尚未整体闭环。r8 及更早候选保持历史，不跨候选转移证据。

历史 r9 候选路径 `output/releases/rust-0.2.0-core-20261023-r9`。Personal 主 SHA `2b5125e60e8652a60255eabb3cb68691723a64b8b292d48fc3799ad8a6f4dca7`，Public `7e70e9e36033d3bb5053d73e57046e89cb57fa13cb3e315dea8a31c371779bc7`。两版 ZIP SHA 分别为 `a17115657dcacf7b2359dde87ccb134f23fae951b31603b9935ec6fa2097b21a`、`18a19d4efb7b354bc2aa1c918ab37095abfc5733a6cd0c4b73606c4cc61bfabb`。两版构建、78/78 归档断言、26/26 离线命令及新鲜解包后的 app/helper strict codesign 均通过；仍为 ad-hoc、无 Team ID。[staging](../acceptance/evidence/e01/core-r9-release-staging.json)、[归档审计](../acceptance/evidence/e01/core-r9-release-audit.json)、[Root 核验](../acceptance/evidence/e01/core-r9-root-verification.json)。172 个源文件匹配 r9 staging，本轮 15 项源文件变更与冻结清单一致；157 个源文件与 r8 相同。45 个保护文件匹配此前当前快照。[保留核验](../acceptance/evidence/e01/core-r9-source-preservation.json)。Root 四条新鲜集成命令通过：workspace/Public strict Clippy、fmt、diff check；引用定向测试，不计为重跑或新增唯一测试。[集成](../acceptance/evidence/e01/core-r9-integration-checks.json)。

r9 Personal/Public 两版真实 `tauri://localhost` 窗口均完成只读修复预览、取消可见计划、重新预览及离页返回后旧确认移除。正式系统组件缺失时执行按钮禁用；原生预览未填充 `user_changes`，未真实执行 `PreparedRepair`。取消只证明可见 UI 移除；后端取消确认无法从此 UI 独立观察，另由隔离请求状态测试和接线复审支撑。原生视觉为 93/pass；两版退出后进程均不存在。[原生记录](../acceptance/evidence/e01/core-r9-native.json)，JSON SHA `bdafa91ae1f9e4a7cd7086e38e6f10afa8eba9b0c914f27dbf2f1ae8e118ca60`。未修复系统、操作真实账号或启动 Claude；未新增完整 14 项检查、网络 ABA/wake、启动计时、100ms 反馈、资源或长时结论。r8 工件与证据冻结为历史；r7 性能仍仅绑定 r7。正式 F-11 管理员授权成功/取消、签名 XPC、CFPreferences+root 写入和逐项恢复仍 WAIT/NOT_RUN。

r9 普通键盘补记：Personal/Public 进程均匹配各自冻结主 SHA；Tab 聚焦只读准备预览，Return 后出现忙碌状态和实际准备结果。安装/备份门槛保持，两版均已退出。此结果仅覆盖该 Tab→Return 路径。[键盘记录](../acceptance/evidence/e03/core-r9-keyboard.json)。核心页没有可编辑文本输入，地区选择使用固定国家 select；当前核心文本输入范围为 NO_TEXT_ENTRY_CORE_SCOPE。文本编辑仅在广告运营页，整个产品的中文 IME 验证随广告后置，未标 PASS；本轮未切输入法。100ms 反馈仍为 NOT_RUN_INSTRUMENTATION_LIMIT；忙碌状态可见不证明 event→paint 达标。

1. 四包、12 任务、37 项唯一验收保持不变；现行规格仅 v1.2。广告后续排在系统核心完成之后。
2. 历史 r8 本地候选，路径 `output/releases/rust-0.2.0-core-20261023-r8`。Personal 主 SHA `898e842430082409e29e8fc28a1d7558f8d6cae4e132ac478c313a6930f2cd3c`，Public `351973b920214e0dd08169f1eaa2edff4c234009e25c42d19258a083cd4c5554`；两版 ZIP SHA 分别为 `15424e0a9108bfeb89197cbd3fcb283128c32c234611d2e8580fc0ce3db42fc6`、`323ad2ba6823dae053e2c64a5383213a597e2ab110cda60f525d2d7fe294ad66`。两版本地构建、78/78 归档断言、26/26 离线命令均通过；新鲜解包及 app/helper strict codesign 通过，仍为 ad-hoc、无 Team ID。[staging](../acceptance/evidence/e01/core-r8-release-staging.json)、[归档审计](../acceptance/evidence/e01/core-r8-release-audit.md)/[JSON](../acceptance/evidence/e01/core-r8-release-audit.json)、[Root 核验](../acceptance/evidence/e01/core-r8-root-verification.json)。168 个源文件匹配 r8 staging；45 个保护文件与此前当前快照一致，相对最初交接仍为 42/45。相对 r7 仅 guardian.rs、lifecycle.rs、macos.rs 三文件改变，另 165 源不变。[保留核验](../acceptance/evidence/e01/core-r8-source-preservation.json)。五条集成命令均 exit 0：network_epoch 8/8、workspace/Public strict Clippy、fmt、diff check。8 项是重跑，不新增唯一测试。[集成](../acceptance/evidence/e01/core-r8-integration-checks.json)。
3. r3 的两轮环境检查、取消晚读及账号预览仍是 r3 范围内证据；没有启动 Claude。有效取消后，晚读 AX 仍显示已取消、尚未取得报告；两份保存记录间隔 78.379 秒，不代表检查耗时。首次失效索引点击未执行，不计成功。r3 原位操作当时受系统组件与备份收据阻断。此旧证据不转移到 r5。
4. r5 两版正常窗口与只读准备 IPC 均已观察。Keychain 查询使用 `Claude Safe Storage / Claude Key`，返回状态 0、属性计数 1；仅为只读元数据，不证明秘密可访问或登录保持。两版准备页现在仅显示正式安装/后台健康与已验证备份收据两项门禁。r5 两种窗口尺寸下账号控件可经纵向滚动访问；Tab、Shift-Tab 与 Return 焦点路径通过限定观察，视觉评分 95/pass。VoiceOver 尝试因工具 -10005 超时而 INCONCLUSIVE，未证实朗读或光标导航；IME 输入源切换/恢复已观察，但中文组字为 INCONCLUSIVE。Public 的 VO/IME 对应测试未运行。六张已存截图的补充视觉复核显示总览/活动页最小窗口文本可读、活动末条可滚动到达；只覆盖这些截图和状态，其他布局仍未验证。100ms 反馈仍未验证。详见[最小窗口视觉复核](../acceptance/evidence/e03/core-r5-minimum-extra-visual-review.json)。详见 [r5 原生记录](../acceptance/evidence/e01/core-r5-native.md)及[selector 原生观察](../acceptance/evidence/e07/keychain-suffixed-selector-native.json)。
5. r3 缓存启动各 10 次，上界 p95 为 Personal 1086.319ms、Public 1038.712ms；首次启动单列。每版 20 次页面反馈均可见，但 100ms 门槛因仪器限制为 NOT_RUN_INSTRUMENTATION_LIMIT，与签名材料等待无关。r3 固定热后资源窗口已完成，但 Personal/Public 完整性均 FAIL：两版 index141 均有进程成员变更且 CPU 为 null；Public index212 另有 live-attribution failure。有效 CPU 均值/覆盖率为 Personal 0.681434%/99.444681%、Public 0.677462%/99.447613%；完整窗口均值为 null。v1.2 没有 CPU/RSS 数值通过门槛，该完整性失败不代表性能阈值失败，也不使 PF-01 通过。详见 [r3 启动](../acceptance/evidence/pf/core-r3-startup.json)、[反馈](../acceptance/evidence/pf/core-r3-feedback.json)、[资源终态摘要](../acceptance/evidence/pf/core-r3-resource-summary.md)和[机器可读结果](../acceptance/evidence/pf/core-r3-resource-summary.json)。
6. 历史 r7 五项 scoped integration 检查通过：`updates::transport_tests` 6/6、workspace/Public strict Clippy、fmt 与 diff check 均 exit 0。6 次执行包含既有 3 个唯一测试和新增 3 个唯一测试；Root 重跑不增加唯一数。r7 相对 r6 仅 `updates.rs` 改动，其余 167 个源文件不变；Personal 92、Public 80 是 r6 历史执行记录，r7 未重跑，不能归给 r7。详见 [r7 integration](../acceptance/evidence/e01/core-r7-integration-checks.md)/[JSON](../acceptance/evidence/e01/core-r7-integration-checks.json)、[源码保留核验](../acceptance/evidence/e01/core-r7-source-preservation.json)、[更新客户端策略](../acceptance/evidence/e11/core-update-client-policy.md)/[复审](../acceptance/evidence/e11/core-update-client-policy-review.json)与[本地集成裁决](../acceptance/evidence/e11/core-update-client-policy-integration-decision.json)。复审为 COMMENT_SCOPED_STATIC、无发现但不是 approval。正式安装身份、管理员交互、签名 XPC 与恢复仍 NOT_RUN。
7. 历史 r8 Personal/Public 两版真实窗口与只读 GUI→Rust IPC 已通过限定观察；每版返回 14 项环境报告，均为 6 通过、1 提示、7 阻止启动，不代表环境全部通过。账号准备预览完成，但正式安装身份/guardian 健康及完整迁移备份收据继续阻止账号操作。生产只读网络订阅检查入口执行成功，真实网络 ABA/wake 回调未诱发，不能记原生网络变化验收 PASS。两版均正常退出；未启动真实 Claude、操作真实账号或修改网络设置。[原生记录](../acceptance/evidence/e01/core-r8-native.md)/[JSON](../acceptance/evidence/e01/core-r8-native.json)，JSON SHA `7f0a4fa2fa5e8a712b54e26a18a1efabdf72a36699596301e6c52175255529b4`。r8 无新启动计时或性能结论；r7 原生窗口/IPC、缓存 p95 仅为 r7 历史证据，不能转移。 正式 Developer ID/签名 XPC/PKG 安装、真实 A 登录恢复、真实 A→B→A、48h/7d、完整动态隐私和 13 项版权通知缺口仍未完成。F-08 不依赖账号 B；F-09 仍 WAIT_B。
8. r5 N2/N3 限定原生网络观察已结束，但隐私仍 PARTIAL。两版 N2 均观察到关联 socket 元数据；Personal 为 61 个采样、34/18 条 nettop 进程/连接行，Public 为 61 个采样、24/32 条；界面报告均为 5 通过、1 提示、8 阻止，未修复或启动 Claude。N3 两版各采样 31 次；因正式源、公钥与 Apple 身份缺失，按钮禁用且观察器未见连接，结论为 INCONCLUSIVE_EMPTY_OBSERVER，不能证明零外发；Personal 另有 1 个同 UID 元数据覆盖失败。未捕获载荷，主机名、URL、字段、重定向和完整生命周期仍未证。详见 [r5 N2/N3 观察](../acceptance/evidence/e11/core-r5-native-network.md)与[机器记录](../acceptance/evidence/e11/core-r5-native-network.json)。
9. r5 隐私窄回归 8/8 通过、目标 strict Clippy 通过；更新同意分层新增 3 个唯一测试（Core 2、Desktop 1）及两 crate strict Clippy 通过。该组 Desktop 测试预置已完成 Check，不证明完整运行时成功 Check→Download→Install；完整 G3 仍 WAIT。测试源码晚于冻结 r5，不能计入其构建审计。详见[隐私回归](../acceptance/evidence/e11/core-r5-privacy-regressions.md)与[更新同意回归](../acceptance/evidence/e11/core-r5-update-consent-regressions.md)。
10. r5 日志导出限定原生复核为 Personal 17 条、Public 10 条，各含 1 条本轮 N2 检查记录；界面导出前/后、持久化和导出逐项一致，目录 0700、文件 0600、nlink=1。此结果不覆盖全部未来日志来源或完整脱敏范围，详见[记录](../acceptance/evidence/e11/core-r5-native-log-export.md)。
11. 按[最新用户全局指令](../acceptance/evidence/e03/core-accessibility-user-waiver.json)，无障碍专项测试为 SKIPPED_BY_USER_DIRECTIVE，跳过不计 PASS，禁止自动开启旁白或重试；仅新明确要求可恢复该专项。r5/r7 旧旁白尝试保留 INCONCLUSIVE。r7 于 2026-10-23 13:22:25.062 UTC 观察开关 off，后续库存显示 VoiceOver 未运行。保留现有无障碍支持；普通键盘功能、中文输入和 100ms 反馈不因该指令免测。r7 中文 IME 本轮 NOT_RUN；r5 中文组字 INCONCLUSIVE 和输入源恢复记录保持历史。 r5 交互尝试后 publisher 目录 9 个文件 SHA 未变，CmdQ 重开清除临时 dirty 状态；未保存或发布。r5 界面时区变化原因未调查，本轮未写系统时区。详见[历史交互尝试](../acceptance/evidence/e03/core-r5-system-interaction-attempt.md)。
12. 通知材料有界调查确认 11 个 Rust 与 2 个 npm 精确版本均无可补齐的新版权归属材料；13 项缺口全部保持 OPEN，不构成法律审查。见[Rust 精确上游复核](../acceptance/evidence/e12/core-rust-upstream-notice-sources.md)及[JSON](../acceptance/evidence/e12/core-rust-upstream-notice-sources.json)、[npm 精确上游复核](../acceptance/evidence/e12/core-npm-upstream-notice-sources.md)及[JSON](../acceptance/evidence/e12/core-npm-upstream-notice-sources.json)、[汇总调查](../acceptance/evidence/e12/core-r5-notice-gap-fix.md)。
13. r5 资源采样已终止，状态为 `SAMPLING_COMPLETE_EVIDENCE_FAILED`。两版各 213 个样本；固定窗口为索引 30–210、约 1800 秒。Personal 完整性失败 99/104，Public 99/114；两版各有 4 个约 40 秒无效区间，完整窗口 CPU 均值均为 null。有效区间均值/覆盖率为 Personal 0.997675%/97.777909%、Public 0.924794%/97.778272%。独立 Astra 复算与记录一致。完整性失败不是数值性能阈值失败，v1.2 没有 CPU/RSS 数值门槛；PF-01 仍 PARTIAL，48h/7d 未开始。此结果只属于 r5，不转移到 r6。旧 r5 启动协议已被后续修复工作取代且未执行。[资源摘要](../acceptance/evidence/pf/core-r5-resource-summary.json)、[独立复算](../acceptance/evidence/pf/core-r5-resource-final-review.json)、[未执行的启动协议](../acceptance/evidence/pf/core-r5-startup-protocol.json)。
## 实现范围

正式 workspace 保持四个 Rust 包：domain、core、helper、desktop。
CLI 与 guardian 属于 core 的二进制，Personal 发布器属于 desktop 模块。
保留两种发行、14 项检查、账号事务、系统修复、更新和三个广告位。
每台 Mac 的系统接管仍只允许一个发行。

源码安装的两版通过本地重建与管理员安装升级。可信签名源分支保留 Personal 手动更新和 Public 12 小时自动检查及空闲安装条件。
运营页、签名密钥和发布命令仅编译进 Personal。
日常广告内容更新不改变 App 版本。

用户选择先本地广告运营。本轮实现固定私有目录的签名发布与读取。
preview 与 production 分开，发布包含锁、CAS、不可变版本及回读。
它不会向其他用户远程投放。远程托管、CDN 和匿名回读另列 WAIT。
已退出的并行接管、专用时间服务、试用发行、点击统计和多云架构保持退出。

## 已有验证与本轮修复

当前 r9 修复：环境页现显示完整 `RepairPreview`；当前地区修复使用 `region=None`。确认执行绑定界面显示的 `requestId`；取消、失败、离页和晚到结果使旧确认失效。修复第 2 版将请求登记移到 `spawn_blocking` 前，后台任务只完成匹配请求，关闭旧预览延迟登记覆盖新计划的竞态。[缺口审计](../acceptance/evidence/e06/core-repair-preview-gap-audit.json)、[r2 冻结](../acceptance/evidence/e06/core-repair-preview-revision2-freeze.json)。前端/边界测试 10/10，Rust 请求状态测试 5/5；后者使用 `None`，只证明请求记账，不证明真实 `PreparedRepair` 执行。Svelte 为 0 errors/0 warnings，两发行 cargo check 通过；未跑完整 core 测试。复审为 COMMENT/SCOPED_CODE_REVIEW_CLOSED、open_issues=0；LSP/AST 未运行，不构成无条件 approval。[复审](../acceptance/evidence/e06/core-repair-preview-review.json)、[本地集成裁决](../acceptance/evidence/e06/core-repair-preview-integration-decision.json)。合成 Cua 视觉为 93/pass，覆盖填充项目和取消移除；它是合成数据证据。[合成视觉](../acceptance/evidence/e06/core-repair-preview-visual.json)。

历史 r7 两版正常窗口与只读准备 IPC 已完成限定观察，见[r7 原生](../acceptance/evidence/e01/core-r7-native.md)。两版各首次 1 次、缓存重启 10 次，共 22 次观察；缓存 p95 为 Personal 1361.596167ms、Public 1192.971167ms。Personal 首次 1562.399ms、Public 首次 2240.453ms 单列保留，Public 首次超过 2 秒。冻结协议采用 nearest-rank，n=10 的 p95 为最大值；计时包含 Cua/AX 开销，是工具上界，不是 paint、系统冷启动或 100ms 反馈。首对/末对进程 SHA 已核对，中间 20 个 PID 未逐一核对；168 源匹配仅为 r7 观察时记录。此结果只属于 r7，不转移至 r8。[启动结果](../acceptance/evidence/pf/core-r7-startup.md)/[JSON](../acceptance/evidence/pf/core-r7-startup.json)、[冻结协议](../acceptance/evidence/pf/core-r7-startup-protocol.json)。

E01 的两个历史构建错误已修复。
真实 WKWebView 的 UI→Rust IPC、未授权命令拒绝和三槽渲染已留证。
正常 Personal 窗口与 ipc-smoke 探针分别记录，不能相互替代。

账号交换、journal、同卷属性保持、安装/更新恢复及签名策略已有隔离测试。
地区修复新增四个 user_plan=Some 组合用例，关闭了仅测试空用户计划的缺口。
真实 CFPreferences、root 写入、正式双进程身份及系统授权仍未执行。

正常原生窗口曾发现活动页缓存问题；修复后可见两条新旧检查记录。
WebKit Blob 导出曾卡在下载目标处理链。现已通过 Rust 固定私有路径完成原生导出复验。
早期运行证据记录两条活动。最终canonical复验现为Personal9条、Public4条，UI与全部导出消息一致；0700/0600、单链接，未使用WebKit Blob。
证据见 [E03](../acceptance/evidence/e03/验证记录.md) 和 [原生导出 JSON](../acceptance/evidence/e03/native-export-runtime.json)。

r3 原生日志导出记录 Personal 16 条、Public 9 条；每版界面、持久化和导出相等，目录 0700、文件 0600、nlink=1。[证据](../acceptance/evidence/e03/core-r3-native-log-export.json)明确这些既有日志内容不归为 r3 生成；脱敏广度仍由隔离证据覆盖。

e04 后续移除了账号测试创建和启动临时伪 Claude.app 的副作用，定向账号测试 12/12。
诊断记录将相关系统弹窗与测试子进程关联；正式 `/Applications/Claude.app` 未被本轮操作。
详见 [e04 测试宿主隔离](../acceptance/evidence/e04/test-host-isolation.md) 和 [e07 诊断](../acceptance/evidence/e07/claude-damaged-dialog-diagnosis.json)。

本机 release 曾因 strip 后的过程宏 dylib 无法加载而阻断。
受控对照确认 strip=false 可构建，workspace 保留该设置。
历史 canonical 六项本地检查与实际测试执行计数见 [E01 检查 JSON](../acceptance/evidence/e01/final-local-checks.json) 和 [计数 JSON](../acceptance/evidence/e01/final-local-test-counts.json)。workspace 执行 222 次，Public 执行 37 次。两版canonical哈希、ad-hoc签名等级和包结构审计已冻结；正式签名、公证、PKG及系统安装仍未验收。

Public 暂停恢复曾有状态来源不一致缺陷。三个新回归覆盖暂停、恢复与重启状态，随后真实 Public 原生窗口暂停/恢复/重启验证通过。App SHA 与四份 AX 步骤记录见 [e08 原生验证](../acceptance/evidence/e08/native-public-pause-resume.json)。这不证明 transport、下载、安装或真实 12h/sleep-wake 验收。

本机广告运营现已完成 Keychain 签名 preview/production rev5 发布与回读，Personal/Public 均消费三槽内容；完成三个广告位独立导入、当前/下一期六条 campaign 编辑和商家 B/C 确认。后续完成rev8单槽暂停、rev9历史恢复、rev10全部暂停，双客户端消费，广告偏好恢复OFF。较早“加密备份恢复等待口令”描述是恢复前状态，已由上方当前补充更新。旧 root 0755 兼容问题已修复并复核，保留原权限，v2 子目录保持 0700。远程 ETag/CDN/20min 继续 WAIT。

当前补充：E06 无 TUN 写入门控新增定向 4/4、helper 库 23/23、clippy 与 fmt PASS，范围仅本地隔离测试。[证据](../acceptance/evidence/e06/no-tun-write-guard.json)明确真实 root/XPC 为 WAIT；生产仍可能写 root 拒绝收据，不声称零 `/Library` I/O。剔除测试代码后源字节一致，不证明 release 二进制相同。

当前补充：恢复入口自阻断 P1、错误默认值 P2、断链 marker P1 均已在源码和定向测试中闭合；累计 47 项分批不同定向测试、4 项新增唯一测试。静态复审无发现但状态为 COMMENT_SCOPED_STATIC，未跑 LSP/AST，也不是 approval；Root 授权并完成本地集成。r6 集成和归档审计通过；正式安装恢复仍 NOT_RUN。证据见 [修复](../acceptance/evidence/e06/recovery-entrypoint-fix.json)、[复审](../acceptance/evidence/e06/recovery-entrypoint-review.json)、[裁决](../acceptance/evidence/e06/recovery-entrypoint-integration-decision.json)与[r6 集成](../acceptance/evidence/e01/core-r6-integration-checks.json)。

当前补充：AD-10 已在 c7c canonical 完成本机真实 Keychain 恢复、生产 revision 10→11、缺失素材重建和 Personal/Public 双端回读。[证据](../acceptance/evidence/e10/native-final-personal-backup-restore.json)记录PASS_LOCAL_NATIVE；远程源、CDN 与其他设备仍 WAIT。较早“等待口令”的段落属于恢复前历史状态。

当前补充：Personal 反馈候选 `2436e514806c45eed5616e4b3728bedfa4390704ab5863b2bac9f8b9fffe5eb2` 的限定范围原生验证已完成，证据见 [E10 原生观测](../acceptance/evidence/e10/native-backup-feedback.json)和[视觉复核](../acceptance/evidence/e10/native-backup-feedback-visual-verdict.json)。仅广告候选 promote 暂缓；VoiceOver 未运行，c7c 的恢复和资源结果不归属于该候选。

## 构建与使用入口

在项目根分别运行 `bash scripts/build-rust.sh personal` 与 `bash scripts/build-rust.sh public`。
不要使用 `--all-features`，两个发行特性互斥。
当前适用环境为 macOS 27.0.1、Apple Silicon；macOS 13 和 Intel 尚无真机结论。

使用说明：[Rust 使用说明](../usage/Rust使用说明.md)。
恢复说明：[Rust 恢复说明](../usage/Rust恢复说明.md)。
运营说明：[广告运营说明](../usage/广告运营说明.md)。

## 尚未完成的门槛

P1 网络事件覆盖缺口当前为 SOURCE_FIXED_SCOPED_LOCAL_INTEGRATION_PASS_WITH_RUNTIME_LIMITS。此前 GUI/CLI 未注册进程内原生网络回调，网络 A→B→A 可复用旧报告的源码缺口，已完成源码修复、隔离回归、独立复审和 r8 本地构建集成。重叠过滤器共有 26 个不同通过测试；r8 重跑 network_epoch 8 项，不增加上述唯一测试数。生产只读原生订阅检查入口成功，但真实网络 A→B→A 与 wake 回调未诱发，运行时验收仍 NOT_RUN。最终代次采样与 LaunchServices 提交非原子，保留调度/提交边界限制，不保证零竞态或零泄漏。[修复](../acceptance/evidence/e02/core-network-generation-fix.json)、[最终复审](../acceptance/evidence/e02/core-network-generation-review.json)、[集成裁决](../acceptance/evidence/e02/core-network-generation-integration-decision.json)。复审 findings=[]、状态 COMMENT_SCOPED_STATIC，非 approval；裁决仅为 PROCEED_SCOPED_LOCAL_INTEGRATION。初始复审 macos SHA 抄写不全已由最终复审纠正，原始记录保留。核心系统尚未全部完成。

r5 交互尝试后，旁白开关与输入源已恢复；该状态只描述 r5 历史。历史 r8 正常窗口与只读准备 IPC 已有独立证据；r9 新证据范围见当前交接状态。r7 缓存启动保持历史。r5 两种窗口尺寸与准备页键盘焦点路径已完成限定原生观察。r5 VoiceOver 历史尝试为 INCONCLUSIVE（工具 -10005 超时；无可证实朗读或光标导航），中文 IME composition 为 INCONCLUSIVE（仅见字母 nihao，无候选或中文提交）；Public 对应测试未运行。旁白开关和输入源已恢复，9 个 publisher 文件 SHA 不变，Personal CmdQ 重开后临时 dirty 状态已清除。此结果不标 PASS，也不判定产品故障。总览/活动页最小窗口的六张已存截图通过限定视觉复核；不覆盖全部状态，其他页面最小布局仍未验证。100ms 反馈门槛为 NOT_RUN_INSTRUMENTATION_LIMIT，不归入证书等待。r5 N2/N3 限定原生观察已完成，但隐私状态保持 PARTIAL；N3 属 INCONCLUSIVE_EMPTY_OBSERVER，不证明零外发。[原生观察](../acceptance/evidence/e11/core-r5-native-network.md)与[payload 覆盖复核](../acceptance/evidence/e11/core-r5-payload-coverage-review.md)记录范围和缺口。隐私窄回归 8/8、目标 strict Clippy 通过；更新同意新增三个唯一分层测试通过，但完整 G3 仍 WAIT，且这些测试晚于冻结 r5。

历史 r7 更新传输局部补证：生产构造器与 TLS fixture 共用私有 builder，六项生产策略保持原样。新增三个唯一测试，六项传输测试通过；真实请求头包含 User-Agent，实际 new 拒绝 HTTP 与不可信 TLS，拒绝测试均有活端点正控制。TLS 成功链仍使用测试 DNS/CA，不证明生产默认信任成功。正式更新源/pins、原生安装和完整 App 重启、全部动作链及完整 G3 仍未验证。见[共享策略证据](../acceptance/evidence/e11/core-update-client-policy.md)与[本次源码绑定集成](../acceptance/evidence/e01/core-r7-integration-checks.json)。

历史 r8 本地候选，路径 `output/releases/rust-0.2.0-core-20261023-r8`。Personal 主 SHA `898e842430082409e29e8fc28a1d7558f8d6cae4e132ac478c313a6930f2cd3c`，Public `351973b920214e0dd08169f1eaa2edff4c234009e25c42d19258a083cd4c5554`；两版 ZIP SHA 分别为 `15424e0a9108bfeb89197cbd3fcb283128c32c234611d2e8580fc0ce3db42fc6`、`323ad2ba6823dae053e2c64a5383213a597e2ab110cda60f525d2d7fe294ad66`。两版本地构建、78/78 归档断言、26/26 离线命令均通过；新鲜解包及 app/helper strict codesign 通过，仍为 ad-hoc、无 Team ID。[staging](../acceptance/evidence/e01/core-r8-release-staging.json)、[归档审计](../acceptance/evidence/e01/core-r8-release-audit.md)/[JSON](../acceptance/evidence/e01/core-r8-release-audit.json)、[Root 核验](../acceptance/evidence/e01/core-r8-root-verification.json)。168 个源文件匹配 r8 staging；45 个保护文件与此前当前快照一致，相对最初交接仍为 42/45。相对 r7 仅 guardian.rs、lifecycle.rs、macos.rs 三文件改变，另 165 源不变。[保留核验](../acceptance/evidence/e01/core-r8-source-preservation.json)。五条集成命令均 exit 0：network_epoch 8/8、workspace/Public strict Clippy、fmt、diff check。8 项是重跑，不新增唯一测试。[集成](../acceptance/evidence/e01/core-r8-integration-checks.json)。 r8 Personal/Public 两版真实窗口与只读 GUI→Rust IPC 已通过限定观察；每版返回 14 项环境报告，均为 6 通过、1 提示、7 阻止启动，不代表环境全部通过。账号准备预览完成，但正式安装身份/guardian 健康及完整迁移备份收据继续阻止账号操作。生产只读网络订阅检查入口执行成功，真实网络 ABA/wake 回调未诱发，不能记原生网络变化验收 PASS。两版均正常退出；未启动真实 Claude、操作真实账号或修改网络设置。[原生记录](../acceptance/evidence/e01/core-r8-native.md)/[JSON](../acceptance/evidence/e01/core-r8-native.json)，JSON SHA `7f0a4fa2fa5e8a712b54e26a18a1efabdf72a36699596301e6c52175255529b4`。r8 无新启动计时或性能结论；r7 原生窗口/IPC、缓存 p95 仅为 r7 历史证据，不能转移。 正式 Developer ID、公证、PKG、签名 XPC 和系统安装仍 WAIT/NOT_RUN。r5/r6/r7 历史证据不跨候选转移。
真实 F-08 A 保存、重启、恢复和登录尚未执行，且不依赖第二账号 B；F-09 因真实账号 B 缺失为 WAIT_B。InitializeA/AssociateA 安全许可及 Keychain 精确 selector、生产许可和恢复路径已实现。r5 两版准备只剩正式安装/后台健康与备份收据门禁；这不证明真实登录恢复。
真实transport更新源/密钥缺失，只阻塞依赖该源的检查、下载、安装及长时触发验证。
本机 c7c 广告备份恢复及 rev10→11 发布已经通过局部原生验证；反馈候选也完成限定范围原生观测。广告后续工作延至系统核心完成后，远程托管、CDN、其他设备传播仍 WAIT。

旧 canonical Personal c7c82955…52633 / Public e498b14a…c6b59 的 35 分钟资源摘要仍只属于历史版本；r3 的独立资源终态保留为严格完整性 FAIL，完整窗口均值为 null，不升级性能 PASS。r3 仍含 Apple 身份验证旧缺陷；该历史缺陷不归属于 r5。旧 canonical 资源摘要见 [Personal](../acceptance/evidence/pf/personal-final-tree-35min-summary.json) 和 [Public](../acceptance/evidence/pf/public-final-idle-tree-35min-summary.json)。
两种窗口尺寸已有 r5 限定观察；VoiceOver 朗读/光标导航与中文 IME composition 尝试均为 INCONCLUSIVE，Public 对应未运行，尚无有效验收证据。旁白与输入源已恢复；9 个 publisher 文件 SHA 未变，Personal 重开清除了临时 dirty 状态。100ms 反馈为 NOT_RUN_INSTRUMENTATION_LIMIT。r3 资源严格完整性 FAIL 且完整窗口均值为 null；不转移为 r5 结果，PF-01 仍 PARTIAL。 按[最新用户全局指令](../acceptance/evidence/e03/core-accessibility-user-waiver.json)，无障碍专项测试为 SKIPPED_BY_USER_DIRECTIVE，跳过不计 PASS，禁止自动开启旁白或重试；仅新明确要求可恢复该专项。r5/r7 旧旁白尝试保留 INCONCLUSIVE。r7 于 2026-10-23 13:22:25.062 UTC 观察开关 off，后续库存显示 VoiceOver 未运行。保留现有无障碍支持；普通键盘功能、中文输入和 100ms 反馈不因该指令免测。r7 中文 IME 本轮 NOT_RUN；r5 中文组字 INCONCLUSIVE 和输入源恢复记录保持历史。
r5 资源采样已完成但证据完整性失败：两版各 213 个样本，固定窗口索引 30–210、约 1800 秒；Personal 99/104、Public 99/114 失败，各有 4 个约 40 秒无效区间。完整窗口 CPU 均值为 null；有效均值/覆盖率分别为 Personal 0.997675%/97.777909%、Public 0.924794%/97.778272%。独立 Astra 复算一致。完整性失败不是数值性能阈值失败；PF-01 仍 PARTIAL，48小时与7天正式观察未开始。结果不转移到 r6；旧 r5 启动协议已被修复工作取代且未执行。[资源摘要](../acceptance/evidence/pf/core-r5-resource-summary.json)、[独立复算](../acceptance/evidence/pf/core-r5-resource-final-review.json)、[未执行的启动协议](../acceptance/evidence/pf/core-r5-startup-protocol.json)。
13项上游版权通知缺口仍 OPEN。Rust 与 npm 精确版本有界研究均未发现可机械补入的版权归属材料；详见[Rust研究](../acceptance/evidence/e12/core-rust-upstream-notice-sources.md)、[npm研究](../acceptance/evidence/e12/core-npm-upstream-notice-sources.md)和[汇总记录](../acceptance/evidence/e12/core-r5-notice-gap-fix.md)，均非法律审查。法律审查及最终链接 SBOM 为 NOT_RUN。
本轮不把开发工件、合成账号或受控测试通过计为正式发布验收。r5 日志导出仅证明两版各自本轮 N2 记录的界面/持久化/导出一致，不证明未来所有日志来源或完整脱敏范围。[r5 日志导出证据](../acceptance/evidence/e11/core-r5-native-log-export.md)。

## 工作区保护

Git HEAD 保持 `2cf108596a7bb382acbc62c2d1dd9b89e17e1d71`，没有提交或推送。
接手时 45/45 保护文件一致；之后仅三处计划内 P0 修复发生变化。
旧安装、旧服务、代理配置与真实账号资料未被本轮修改。
用户接手前已有 tracked 与 untracked 修改全部保留。
r9 的 45 个保护文件与此前当前快照一致；172 个 staging 源文件及 15 项授权变更匹配，范围见[本轮保留核验](../acceptance/evidence/e01/core-r9-source-preservation.json)。该核验不扩写为整个工作区或登录保持通过。
