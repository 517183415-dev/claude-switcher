# Claude 切换器 Rust 重写 PRD

本轮合成备份IPC源码提交为 `6136fe85fe573e9ebe42316f8e482070e9c34127`，tree `292a93f1013c725bc0579f457eb3cf60039008ee`。此前 `b4d06baa2494d55247d5a41e13a7702cc27c7b23`补足偏好故障与专用r7测试；`3a8161ea0a7ab0acffd21e0ef04cc6de73c3385d`使严格解析器单列ignored。[native-r3回执](../../output/no-account-20261027/encrypted-backup-ipc/native-r3/summary.json)与[独立复核](../../output/no-account-20261027/encrypted-backup-ipc/review-native/review.json)绑定当时的112项输入内容图，不证明完整6136源码构建。其中 `e11_log_privacy.rs`仍为后续Clippy修正前内容，三个components二进制属于生成输入；112项不能统称git源码。该e11修正由后续Rust r2与专用r7验证。89项runner回归包含18项备份解析测试。Personal使用共享恢复body和MemoryVault完成1成功/4拒绝：错误口令、篡改、截断及Vault故障。每次在同一fixture进程重开Publisher检查状态；Public两个备份命令均在权限层拒绝且无写入，不能由此证明运行时handler不存在。

备份失败保护活动状态、旧代次及旧Vault条目；不要求整个目录树不变，可能保留非活动孤立代次。本轮合成备份不含图片资产；四独立进程证据仍只证明选定偏好持久化。native-r1目录初始化失败与native-r2旧Public oracle失败均保留；目录fixture有Rust RED/GREEN，权限oracle有2项RED及18项GREEN。MemoryVault不证明生产原始Keychain命令、备份跨进程恢复、普通App或完整资产覆盖；独立WebKit子进程清理仍未证明。

[Rust r2严格对账](../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/strict-result.json)确认705顶层passed/601个passed名称、1 ignored，合计706记录/602名称；3条nested不重复计数，按harness清单一致。ignored活动记录写拒绝测试需单层Seatbelt，已由[专用r7](../../output/no-account-20261027/ordinary-next/preferences-faults/r7/receipt.json)独立通过。该直接Rust测试覆盖两版：偏好目标阻塞时保留活动记录；活动记录写拒绝时返回错误，已提交偏好可由新状态读回。它描述部分提交行为，不证明双文件事务，也不属于原生IPC。tests/list/fmt/默认Clippy四命令exit0，[两版ipc-business feature Clippy](../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/ipc-business-clippy/summary.json)通过；原粗计数失败回执及此前失败轮次保留，不把ignored计入passed。当前[图谱](../../output/no-account-20261027/encrypted-backup-ipc/codegraph-final.json)为183文件/4997节点/19474边，complete、pending0。当前普通双包已从6136精确git archive构建；[交付回执](../../output/no-account-20261027/bundles-backup/delivery-summary.json)绑定完整提交 `6136fe85fe573e9ebe42316f8e482070e9c34127`、tree `292a93f1013c725bc0579f457eb3cf60039008ee`及archive SHA256 `ddcf3bc6490e466c163cf290727639b99ee3171abbe93da1d47a717e8d2e4fbe`。每版22项静态检查及2项复制/签名变更完成，各12条目重验通过；备份fixture运行体标记未检出。E01保留REVIEW；[独立包复核](../../output/no-account-20261027/bundles-backup/review/review.json)已通过限定静态验证。Public当前dist已独立重扫；Personal原始dist已被覆盖，只能引用构建时dist证据。普通App启动、动态ACL与安装仍NOT_RUN。旧d0a及更早包保留为历史。普通App、动态ACL、安装、真实账号、真实Keychain、远程发布及系统/长时门槛保持未验。

历史9f跨进程偏好与共享MemoryVault提取绑定源码提交 `9f0c906776d7ff30adea6d956484d764fd262715`，tree `fc28bbcd4f83ff0d4b50c1349cad1dd6401903e3`。[跨进程原生回执](../../output/no-account-20261027/preferences-persistence/native-r1/summary.json)与[独立复核](../../output/no-account-20261027/preferences-persistence/review-native/review.json)证明两版共4个独立fixture进程：Personal/Public分别写入，再各启新进程读回选定非默认偏好；版本间文件哈希保持，最终4目录0700、4文件0600且单链接。71项runner回归通过，109项构建输入绑定。它只证明受控入口的选定设置跨进程持久化；完整组合、崩溃/断电恢复、同用户恶意文件竞态及普通App仍未验。

本轮Rust r1的tests/list/fmt/Clippy四命令exit0；[Rust r1严格对账](../../output/no-account-20261027/preferences-persistence/rust/r1/strict-result.json)确认704顶层执行/600名称/3嵌套，清单多重集一致，嵌套不重复计数。原粗计数失败回执保留，不与此前r3混为同一轮。历史跨进程阶段[图谱回执](../../output/no-account-20261027/preferences-persistence/codegraph-status.json)为178文件/4887节点/19118边、pending0；下方175/4835/18966为历史快照。该历史轮次普通双包精确绑定d0a2498；该轮未构建9f0c906普通包，普通App启动、动态ACL、安装、真实账号与Keychain保持NOT_RUN。

历史d0a偏好阶段[双包独立复核](../../output/no-account-20261027/preferences-ipc/review-bundles/review.json)已完成：两版现存App的12条目/7文件、日志及精确归档绑定匹配，二进制/resources运行体扫描复现。Public当前dist可独立重扫；Personal原始dist已被覆盖，仅保留历史dist哈希/扫描回执。不能由二进制扫描重建原始压缩前端输入。E01维持REVIEW，普通App启动、动态ACL与安装仍NOT_RUN。

历史d0a偏好阶段普通双包：[交付回执](../../output/no-account-20261027/bundles-preferences/delivery-summary.json)与[构建回执](../../output/no-account-20261027/bundles-preferences/final-summary.json)精确绑定产品/fixture提交 `d0a2498092f1e6b5cea16b8c30e7b811ead9493b`，不绑定后续文档提交。原构建每版22条静态检查及2条复制/签名变更通过，Public后各12条目重验通过；独立审查已完成限定静态复核。business/limits/偏好快照fixture运行体标记未检出，历史E01保留REVIEW_MARKERS_PRESENT。普通App动态ACL、启动、安装仍NOT_RUN；旧00db包完整保留。

历史d0a偏好IPC源码/fixture提交：`d0a2498092f1e6b5cea16b8c30e7b811ead9493b`，tree：`7fbc4df5d04708c87d6b44486ae46d06825248e3`。两版各22次真实save_preferences调用，5次成功/17次拒绝，各44份前后快照；所有拒绝均保持两个文件不变。106项输入绑定，53项runner解析回归通过。[原生回执](../../output/no-account-20261027/preferences-ipc/native-r1/summary.json)与[独立复核](../../output/no-account-20261027/preferences-ipc/review-native/review.json)限定受控独立入口，不运行普通启动；该历史轮次未验重启后偏好持久化；本轮仅补足上方受控跨进程范围，完整矩阵及普通App仍未验。53项是runner回归，不与证据对账解析器25项混计。

Rust r3四命令exit0；原正则计数失败保留。[仓库严格解析器对账](../../output/no-account-20261027/ordinary-next/rust/r3/strict-reconciliation.json)确认704顶层/600名称/3嵌套，按harness清单一致，嵌套不重复计数；它是r3既有执行的对账，不是额外Rust运行。历史d0a普通双包已从d0a2498精确归档构建并完成原构建静态检查，独立审查已完成限定静态复核；旧00db包及原哈希保留。历史偏好阶段[图谱](../../output/no-account-20261027/preferences-ipc/codegraph-status.json)为175文件/4835节点/18966关系，complete、pending0；旧175/4814/18877为历史快照。

解析器提交为 `ac1d48b49a72df6d83b7e08c05fc6406b3f7e2b8`，tree：`cbe31f21bd652991ada53c26a7507bec382d32d4`。`tests/evidence_reconciliation/`的25项回归及[独立审核](../../output/no-account-20261027/ordinary-next/review-rust-r2/portable-review.json)限定通过；仅支持当前串行libtest/F06三子进程输出格式，不支持任意并发或新嵌套格式。历史回放严格核对704顶层/600名，原失败summary保留；解析器核验证据一致性，不认证回执或重跑Rust。解析器交付阶段的历史普通包绑定00dbf3c，不改标为解析器提交。此前[图谱状态](../../output/no-account-20261027/ordinary-next/codegraph-status-final.json)为175文件/4814节点/18877关系，complete、pending0。

历史00db普通双包独立复核结论为PASS_STATIC_ARTIFACTS_WITH_PROOF_LIMITS：每版12个现存条目/7文件及24份命令日志哈希重新核对，commit/tree/archive均匹配。该历史轮次的临时frontend dist已清理，该次复核不能独立重扫，仅保留原构建扫描回执绑定。普通App动态ACL、启动和安装仍未验；解析器已完成限定独立审核。见[00db历史普通包独立复核](../../output/no-account-20261027/ordinary-next/review-bundles/review.json)。

历史00db普通双包已构建：绑定 `00dbf3c` 的精确归档，每版22条静态检查及2条复制/签名变更通过，Public构建后两版各12条目重验通过。business/limits fixture运行体标记未检出；历史E01权限描述字符串保留REVIEW_MARKERS_PRESENT。普通App动态ACL、启动及安装仍NOT_RUN。独立包复核已完成限定静态核对；解析器已完成限定独立审核。[双包交付回执](../../output/no-account-20261027/bundles-ordinary-next/delivery-summary.json)记录准确路径与哈希。

当前发布命令生产实现仅装配LocalObjectStore，使用本机私有文件；尚无已装配的生产HTTP ObjectStore。广告受控HTTPS客户端及MemoryStore状态机测试不证明发布HTTP CAS已实现或通过；远程对象存储适配与实源验收单列未完成。

历史00db普通包产品源码提交：`00dbf3c486b65aa5840033954485af2262fb85d3`，tree：`a756309a1e69e6754fd8d40e6ba33770fce226ba`。最终Rust r2的tests/list/fmt/Clippy四命令均exit0。原summary计数失败原样保留；独立按执行文件与嵌套harness分组后，为704个顶层通过记录/600个名称，按(binary,name)与清单多重集合一致，3个恢复子进程不重复计数。该复核读取既有执行记录，没有重跑Rust测试；后续解析器提交单独记录。两项原生排除不变。见[计数复核](../../output/no-account-20261027/ordinary-next/rust/r2/reconciled-summary.json)和[独立审查](../../output/no-account-20261027/ordinary-next/review-rust-r2/review.json)。普通双包已从本提交精确归档构建并完成静态检查；CNC及旧包历史绑定保留。

本阶段增量（进行中）：三文件仅测试提交 `30e861aca3c794d521f921d24e1dd112827bddb9` 的统一隔离组700次顶层执行/596名通过，新增8名无删除；list一致，fmt、Clippy通过，同两项原生排除。它与692/588及CNC684/580分轮记录，不包含子进程重复计数。两版已通过受控独立入口调用真实活动记录/导出命令的原生IPC；它直接注入合成DesktopState，不运行普通启动。显式代理407/502合成端点两项新场景及三个普通切换SIGKILL→全新进程恢复定向回归完成。生命周期Fake、注入TLS客户端与合成备份Vault的证据限制继续保留。稳定runner r9已完成限定验证；最终Rust全量r2已完成并经独立分组复核；历史00db普通双包已完成限定静态验证及独立静态复核。真实账号、正常App、实源、系统、安装与长时仍未验。详情及原始回执哈希见[本轮独立证据索引](../acceptance/evidence/no-account-20261027/ordinary-next-index.md)。原37项验收行保留，35项完整性未证明，33–66小时粗估保留。

稳定runner r9：两版fixture构建及preflight/business均通过，40项回归通过，106项输入哈希绑定。两版实际沙箱负控均通过；4项合成credentials/config读取被拒绝。路径、HOME、链接及profile相关验证保留早期失败尝试，最终40项包含对应负向回归并通过；早期组合失败含helper缺失errors，不作为四类缺陷各自独立有效RED的证明；失败回执保留，旧PASS只属于旧快照。[r9回执](../../output/no-account-20261027/ordinary-next/business-ipc/stable-r9/summary.json)仍限定真实活动记录命令的受控独立入口；历史00db普通发行构建已完成静态检查及独立静态复核，最终Rust全量r2已完成并经独立分组复核，不据此标普通App通过。

测试补足轮次：仅测试提交 `e4f8cb0c87e27ce0e95758aec5abab57c76d4aa3`（tree：`ef5a8831e7009b3ec9c76098d76a8a75418200e2`），四文件仅改测试，无新产品bug。Rust本轮692执行/588名/0失败，新增8名无删除，list一致，fmt及全部目标Clippy通过；仍排除原有两项原生测试。它与CNC原轮684/580分开计数。普通产品及双包仍绑定CNC `23c9964`；原生六边界与独立feature测试1PASS未重跑。[本轮输入复核](../../output/no-account-20261027/branch-audit-next/after-test-commit.json)已核对95个稳定构建输入：92个已提交输入逐项匹配git blob，另3个为既有ignored派生组件。Python7、前端73、TypeScript49及commands/bridge静态读取均重新核对哈希，匹配原成功snapshot；Python200与前端63+2按该范围限定复用，未重跑。G1/G2补足未确认、关闭缓存及拒绝持久化矩阵；Publisher补足三槽两期编辑/暂停/失败/重启、历史恢复序号和单侧凭据；G3补足完整wire捕获、正控制及4097拒绝。真实provider与35项完整覆盖仍未证明，33–66小时粗估保留。详见[分轮进度](../handoff/无需账号功能进度-2026-10-27.md)及[测试补足机器摘要](../acceptance/evidence/no-account-20261027/branch-audit-followup.json)。

版本：v1.2。初版日期：2026-10-22，Asia/Taipei。更新日期：2026-10-27，Asia/Shanghai。
原交接：CS-RUST-20261022-R4。本地整合：CS-RUST-INTEGRATE-20261027。
状态：本地代码整合完成，产品实施与验收继续进行。交付方式为 GitHub 源码与本地构建／安装脚本。

`codex/gpt-audit-fixes` 的产品代码已全部整合到 `codex/rust-rewrite`。整合基线C49为 `49c2615beac0e1f7ac7352977d3212229fe9b012`，交付记录提交为 `84d7bc1b0b0d80630745272361b6aaa41c9259fa`。历史产品修复CNA为 `0297efe2da236427b9a95a584aba53c4f30ce634`，包含检查截止、证据有效期、启动前网络复核和测试时钟隔离。历史产品CNB为 `7851ce10bd00a040bf8c209fd6e763baf63546ff`，tree为 `27d4826b204d079f3a28c48a7f2089d31fe1992e`。CNB修复CLI普通Fix成功apply后恰好一次完整14项复检，以及悬空current符号链接拒绝。C49/C4D/CNA旧包及历史证据全部保留。

历史已打包产品CNC：`23c9964403e904ab44c2cc334d40992454193221`，tree：`920c47bf92fc69ce8916266f61327b3766c8fada`。CNC增加仅编译期启用的ipc-limits独立main/page/config，并提取共用import_ad_image_body，保留生产先限长、再识别MIME、再导入的顺序。

2026-10-27 用户明确选择本PRD的启动合同：证据超过30秒或网络变化后重新检查，启动前再次复核。该决定覆盖旧版恢复计划的对应条款。真实原生网络事件、系统启动和长期运行仍需各自取证。

| 当前结果 | 证据边界 |
| --- | --- |
| R01–R12、A01–A03 | 已整合至C49；CNB继承全部成果，具体结果以整改记录为准 |
| 历史Personal/Public完整App | C49两版44项静态检查为历史结果；CNB双包从固定提交重建，每版24条打包后命令通过，其中22条为静态检查，准确路径和哈希见本轮记录；两版均未安装或启动 |
| 历史CNB/v5隔离回归 | Rust本轮新执行679次执行、575个不同名称、0失败，新增8方法；fmt与Clippy通过。Python200、前端60+2及类型检查复用，未新跑；7个Python输入哈希未变。CNA/v4的671/567保留为历史 |
| CNC当前结果 | Rust本轮684执行/580名/0失败，另列feature测试1PASS；fmt和各Clippy通过。前端63+2、Svelte0/0按73前端/49TS输入一致复用；Python200按7输入未变限定复用，未新跑。普通双包独立构建并静态验证；每版22检查+2复制/签名变更，Public后两版各12条目重验PASS；未安装/启动 |
| CNC A01原生边界 | 两版隔离preflight与Personal六个载荷边界限定通过；图片实际import，备份仅closure计数，无解密/Vault。整项A01、普通GUI/IPC仍未验 |
| 实际安装、真实账号、外发与长期运行 | 尚未完成对应验收；不能由构建或隔离测试推定通过 |

本轮结果、候选绑定、排除项和剩余工量见[无需账号功能进度](../handoff/无需账号功能进度-2026-10-27.md)。整合阶段结果见[本地整合交付](../handoff/本地整合交付-2026-10-27.md)。当前CNC机器证据见[原生后续摘要](../acceptance/evidence/no-account-20261027/native-followup.json)。CNB历史机器证据见[后续摘要](../acceptance/evidence/no-account-20261027/branch-followup.json)。有限范围审查仅有限推进，不代表35项逐分支完整覆盖；33–66小时粗估未因此缩减。37项标准以[候选验收](../acceptance/当前候选验收-2026-10-27.md)为准，未将局部测试升级为完整验收。[原始整改证据](../handoff/当前整改证据-2026-10-25.md)保留历史哈希与分层结果。[当前代码图谱](../design/code-map.md)说明模块边界。

> 执行顺序沿用 2026-10-24 决定：先完成旧版已有的核心功能与必要安装恢复，再推进广告运营及其他扩展。保留全部现有成果。2026-10-25 的账号、修复和守护合同继续有效。完整产品范围保留；本地整合不代表全部 37 项验收通过。本轮未安装，也未使用真实账号。

## 1. 产品目标和首版范围

交付 macOS 桌面应用。使用 Rust 核心、Tauri 2 和 Svelte 5/TypeScript。沿用已确认的暖白、墨绿、横向工作台。

| 范围 | 首版要求 |
| --- | --- |
| 核心使用 | 14 项检查、检查后启动、单账号初始化、A/B 档案、诊断、修复、地区设置、后台守护 |
| 两种发行 | Personal 自用；Public 对外分发；一套源码，两个固定发行配置 |
| 三广告位 | 美国家庭宽带、注册好的账号、KYC 服务；各自编辑、开关、排期和暂停 |
| 运营管理 | Personal 提供草稿、预览、签名发布、回读、历史恢复和加密备份 |
| 系统与交付 | 安装、授权取消、卸载、迁移、回退、外发请求验收和真实运行证据 |

首版面向当前 Mac。优先验证 Apple Silicon、0dcloud 和 Clash Verge。macOS 13 与 Intel 的支持声明由真实验收决定。Windows/Linux、额外代理客户端、多商家登录、交易结算和多账号扩展留待独立需求。

本次交付源码、锁文件、构建脚本、可恢复的本地安装脚本和中文说明。用户无需购买 Apple 开发者会员。商业签名、公证和托管二进制发布不属于本次交付门槛。先完成核心功能，广告运营的后续工作保留并后置。

每台 Mac 安装一个发行，并登记一个 macOS 管理用户。Personal/Public 的替换走显式安装迁移。两个发行可以复用已验证的数据格式，分别保存发行配置和密钥。该约定替代同机并行接管设计。

## 2. 两种发行和资料去向

安装方式决定软件更新路径。SourceLocal 的 Personal/Public 均通过“拉取源码 → 本机构建 → 查看计划 → 管理员安装”升级。两版均不自动检查、下载或安装远端更新，也不将未配置的自动安装显示为可用。

下表前三行适用于 DeveloperId 安装及已配置的可信签名发布源。Public 保留这一能力。广告展示和运营管理按发行划分。

| 行为 | Personal | Public |
| --- | --- | --- |
| DeveloperId 软件检查 | 本人点击后执行 | 默认每 12 小时执行；提供暂停和手动检查 |
| DeveloperId 下载和安装 | 本人分别触发；每次安装确认 | 可自动下载并验证；Claude 完全退出、写入空闲且组件兼容后安装；允许延后 |
| DeveloperId 来源与签名 | 本人控制发布源与签名 | 产品发布者控制正式源与签名；用户能查看版本和更新记录 |
| 广告展示 | 默认关闭 | 首次说明请求用途后，按用户设置读取 |
| 运营管理 | 本机管理三个广告位 | 安装包包含读取、公钥和展示功能 |

账号档案、聊天、Cookie、登录令牌及设备资料在本机处理。诊断与日志先脱敏，再由用户导出。切换器所有外发请求按触发、目标、载荷和响应用途验收。

远端广告和地理信息按固定数据结构解析。软件变更通过已认证的软件安装流程交付。广告发布权限与软件发布权限分别管理。商业内容点击交给系统浏览器。

可信源的 Public 更新可以向客户端交付新程序代码。首次说明和设置页写明发布者、检查周期、安装条件与暂停入口。

## 3. 核心用户流程

### 3.1 单账号与 A/B 档案

初次进入时识别当前 Claude 数据和已有 A/B 档案。首次非空活动资料沿用收编 A 合同。归属未知且活动目录为空或缺失时，保留已有 A。默认显示 A 不证明活动资料归属；不安全路径和冲突事务保留资料并停止。

档案状态固定为：未建立、已保存、当前使用、恢复待处理。登录状态仅根据实际可取得的证据展示。

保留 accounts/A 与 accounts/B。安全有效的 current A/B 标记存在时优先读取。标记缺失或内容不识别时兼容 v2/active-state.json；不安全文件拒绝；需要回退 v2 时，损坏记录拒绝。旧、新事务各按其 journal 恢复，不能将 current 一概当作派生镜像。

GUI 选择账号与 CLI switch A|B 均先检查环境。通过后完全退出 Claude，再保存和交换资料，随后复检并请求启动。另保留当前账号的检查并启动入口。环境检查失败时不启动。错误页面显示原因和恢复入口。

账号切换沿用旧版目录存档，不增加 Keychain 调查或迁移备份前置门槛。准备接口仅保留兼容诊断，备份入口明确不支持。真实登录与 A→B→A 仍需实际账号验收，本轮保持 NOT_RUN；相关长期验收保持 WAIT。

### 3.2 14 项检查与启动

| 稳定 ID | 中文名称 | 策略 |
| --- | --- | --- |
| proxy_process | 代理客户端 | 必需 |
| tun_route | 隧道路由 | 必需 |
| egress_region | 出口地区 | 必需，核对已保存目标 |
| ip_continuity | 出口地址变化 | 提示 |
| timezone | 时区 | 必需 |
| locale | 地区格式 | 必需 |
| ipv6_policy | IPv6 策略 | 必需 |
| dns_route | DNS 路径 | 必需 |
| stun_path | STUN 路径 | 必需 |
| guardian | 后台守护 | 必需 |
| service_routes | 服务路由 | 必需 |
| client_language | 客户端语言 | 必需 |
| asn_info | 网络运营商信息 | 提示 |
| service_transport | 服务传输与访问结果 | 必需 |

每次报告含 14 个完整、唯一的 ID。必需项失败、超时、缺失或证据过期时，启动进入等待。提示项显示具体原因。

检查网络变化后，重新取得相关证据。STUN 全超时显示“待确认”。401 显示需要认证，403 显示访问限制。必需项核对 TLS 与传输结果；访问与账号状态分别展示。账号可用性通过真实登录流程验收。

目标检查预算为 30 秒。必需项超时仍返回完整报告。检查完成到启动的证据年龄上限为 30 秒，期间网络代次应保持一致。

### 3.3 诊断、修复和守护

独立诊断接受明确代理配置，输出 DNS、TLS、HTTP 与超时结果。诊断默认采用自身的代理上下文，并隐藏代理鉴权内容。

普通一键修复在一次点击内生成计划并执行，成功后立即全检查。地区变更先保留用户选择并确认计划。系统动作仍按权限请求授权；失败或取消不标完成。恢复入口继续读取既有事务。

用户级 guardian 在网络事件后复核，另设十秒兜底。停止路径独立于账号复制。实际检测和退出时延须运行取证，当前不保证五秒停止。用户 guardian 保留健康、事件和受信 Public 调度，不直接执行 systemsetup。

root helper 只执行固定系统操作，保留授权校时能力。root 时区看门狗保留 RunAtLoad 与每小时校正。地区、网络服务和路径经校验。旧服务在安装时交接，恢复时按可信备份处理；本轮未运行实际服务。

源码本地安装由管理员登记本次 GUI、guardian 和 helper 的代码指纹。安装记录受 root 权限保护。重建或发行替换必须重新走完整本地安装事务。普通应用请求不能登记新的可信指纹。身份、权限、服务健康和恢复验证仍是本机接管的条件。

## 4. UI 与交互

视觉依据：[已确认概念图](../design/ui-concept-v1.png)。

| 区域 | 要求 |
| --- | --- |
| 窗口 | 主验收 1180×800；最小验收 960×680；主内容单一纵向滚动 |
| 导航 | 总览、账号、环境、活动、设置；Personal 增加运营入口 |
| 总览 | 首屏显示当前账号、检查摘要、当前问题和启动主操作 |
| 环境 | 14 项状态、具体证据和可执行修复入口 |
| 广告 | 总览底部三个独立卡位；功能错误、授权和恢复操作保持可见 |

采用中文状态与错误信息。提供键盘操作和清晰焦点，保留已有语义标签。用户已取消无障碍专项测试；不再启动 VoiceOver 或切换辅助功能设置。ASN 暂缺时显示“网络运营商信息暂未获取”。摘要按实际状态显示“13 项通过 · 1 项提示”。

各页面覆盖加载、空数据、成功、失败、等待材料和恢复状态。异步操作显示当前步骤及可用的取消动作。

## 5. 三广告位和后续运营

### 5.1 三个固定卡位

| 顺序 | ID | 名称 |
| --- | --- | --- |
| 1 | residential_broadband | 美国家庭宽带 |
| 2 | account_service | 注册好的账号 |
| 3 | kyc_service | KYC 服务 |

三个 slot 始终完整、唯一。每位支持当前一期和下一期，合计最多六条 campaign。每期保存商家、标题、说明、图片、按钮、HTTPS 链接、起止时间和开关。

时间保存为 UTC。编辑器默认读取本机时区，允许切换。重叠排期必须在发布前解决。常规内容与商家链接更新沿用现有软件版本。

### 5.2 运营操作

1. 选择广告位，编辑并保存本地草稿。
2. 用客户端同一组件预览两种窗口和下一期。
3. 发布到 preview，检查完整内容与目标链接。
4. 签名并发布到 production，回读版本和素材。
5. 按需暂停、续期，或使用历史快照发布更高序号。

```text
Personal 草稿 → 同组件预览 → 签名发布 → 自控静态内容源
                                              ↓
                                 Public 验签 → 本地排期 → 展示
```

素材采用静态 PNG/JPEG。单张导入与下载上限均为 1 MiB，包含边界值；像素上限 200 万。签名清单上限 256 KiB。

本地索引最多保存 256 条历史和 128 个素材。手动整理默认保留 128 条历史，并保护草稿、pending、两份认证 current、保留历史和终态记录的引用。历史不按天数自动删除。索引整理与素材回收分别预览、分别确认。未知文件和其他代际不在回收范围。未完成批次可以重启后继续。

广告签名私钥、发布凭据存入本机受控密钥存储。加密备份容器上限为 64 MiB，包含边界值。备份不携带正在执行的整理批次；目标须先结束该批次，再恢复备份。Public 包包含公开验证材料。首版仅记录运营发布历史，点击和曝光统计留待独立需求。

### 5.3 刷新、撤回与异常

| 场景 | 行为 |
| --- | --- |
| 正常在线且至少一位开启 | 每次成功后 15 分钟，加当周期一次 0–60 秒随机延迟 |
| 启动或唤醒 | 到期补查；未到期继续剩余计划 |
| 健康在线传播目标 | 连续运行、源健康、首轮成功时，公开回读后 20 分钟内采用 |
| 离线 | 展示至“最后成功+24小时、广告结束、清单到期”中的最早时刻 |
| 关闭全部广告 | 停止广告清单与素材请求；手动运营操作单独说明目的 |

客户端按系统 UTC 排期，并检查本地时间连续性。发现明显回拨或时钟跳变时，收起广告并暂停软件安装。用户校时后，执行“重新建立时间基线”，再获取并验证内容；已有最高序号保留。

暂停广告通过发布更高序号完成。运营页显示在线刷新条件和离线期限。签名失败、超限或过期时，使用仍有效的缓存或本地占位。广告异常单独显示，核心账号功能继续按自己的检查结果工作。

## 6. 交付与验收

工程细节、任务和 37 项验收统一放在 [工程执行文档 v1.2](工程执行文档-Rust重写-v1.2.md)。本 PRD 记录产品要求与交付边界。

当前已完成CNC适用Rust隔离回归及限定原生载荷边界。普通CNC双包已独立构建并完成静态验证，准确路径、哈希及REVIEW_MARKERS_PRESENT的限定解释见[当前CNC候选绑定](../handoff/无需账号功能进度-2026-10-27.md#0-当前cnc阶段)。下一步继续普通业务IPC和受控传输取证。实际安装、真实账号与长期运行仍按各自条件推进。

| 阶段 | 完成要求 |
| --- | --- |
| 开发可运行 | 实际 Tauri 窗口、IPC、类型生成、只读检查和隔离数据流程有证据 |
| GitHub 源码交付 | 两发行可构建；本地安装／恢复脚本和中文说明可用；已测环境与未完成验收明确列出 |
| Personal 本机使用 | 单账号、守护、修复、安装恢复和自身外发策略完成真实验收 |
| Public 完整功能验收 | 全部适用 F/PR/AD 验收、实际安装身份和真实 A→B→A 通过；源码安装的更新边界单列 |
| 稳定性声明 | 候选通过后完成 48 小时，再完成 7 天观察；未完成时不作稳定性声明 |
| 平台支持声明 | 按实际通过的系统、架构、代理适配器列出 |

Personal/Public 各自记录适用结果。37 项验收 ID 保留，源码交付与功能验收分别记录。商业签名分支不阻塞源码交付；广告托管、第二账号与长期观察只影响对应结果。无障碍专项记录 SKIPPED_BY_USER_DIRECTIVE。

维护实现遵循 DRY、SOLID、高内聚和低耦合。共享业务规则留在核心模块，界面通过明确接口调用。文件按领域与职责归类，优先复用现有实现。

核心业务测试覆盖正常、边界、失败、取消、权限与适用的并发和中断恢复场景。每个发现的 bug 必须增加可复现的回归测试，再修复并验证。UI 层不新增强制单元测试要求；既有功能验收继续有效。

首版保持两个发行。单账号试用作为 Personal 的使用场景记录。后续新增发行、运行服务、跨平台或统计功能时，先明确需求和成本。

