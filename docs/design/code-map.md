# Claude 切换器当前代码图谱

本轮资产与代理源码提交：代理 `1836db7f59867bb437c3938610e8d6b603c820a8`；合成PNG资产恢复 `89619059dd473ad04503fa3326476d0526980016`，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`。[代理回执](../../output/no-account-20261027/controlled-proxy-tls/final-receipt.json)记录生产client builder等价提取及受控CONNECT/TLS验证。合成CA只在cfg(test)替换信任，不改变生产默认信任；本机受控传输不证明真实代理或账号可用。r1及isolation-red系统编码环境失败保留：macOS在env_clear后生成 `__CF_USER_TEXT_ENCODING`，fixture随后仅放行该编码提示；此失败不称为产品传输缺陷。

[资产native-r2](../../output/no-account-20261027/backup-assets-ipc/native-r2/summary.json)与[独立复核](../../output/no-account-20261027/backup-assets-ipc/review-native/review.json)证明Personal通过共享恢复body及MemoryVault恢复真实合成1×1 RGBA PNG：1成功/4拒绝、同进程5次重开，最终源/恢复/旧资产的字节、CRC、受限解压、像素、哈希、类型、大小、尺寸及引用独立核对，附加元数据已移除。121项解析器回归包含16项新增资产检查。四个失败场景的不变性来自fixture；只有最终磁盘独立检查。120项历史输入内容图绑定该轮，不能称为完整当前git源码；独立复核时 `network.rs`与 `tests/fixtures/explicit_proxy_tls.py`已有后续漂移。r1虽Cargo exit0，但因构建中输入漂移拒绝二进制归属，失败证据保留。历史parser/red是ImportError，不是行为RED。

[本轮Rust r1严格对账](../../output/no-account-20261027/asset-proxy-verification/rust/r1/strict-result.json)确认707 passed/603个passed名称、1 ignored、3 nested，按harness清单一致；四命令exit0，[两版feature Clippy](../../output/no-account-20261027/asset-proxy-verification/rust/r1/ipc-business-clippy/summary.json)通过。与先前705轮次分开，ignored及nested不重复计入passed。当前[图谱](../../output/no-account-20261027/asset-proxy-verification/codegraph-status.json)为189文件/5130节点/20104边，complete、pending0。当前普通双包已从 `89619059dd473ad04503fa3326476d0526980016` 精确归档构建，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`；[交付回执](../../output/no-account-20261027/bundles-asset-proxy/delivery-summary.json)绑定archive SHA256 `7bc9e7dcc4551d25976d53df1ad19d33a5f696c9b558aa87ea52152d8402fa94`。每版22项静态检查及2项复制/adhoc签名操作完成，最终重验通过；fixture运行体标记排除，E01为REVIEW_MARKERS_PRESENT。[最终独立包复核](../../output/no-account-20261027/bundles-asset-proxy/review/final-review-v2.json)通过限定静态检查。Public当前临时dist可独立重扫；Personal原dist被Public构建覆盖，仅保留构建时dist回执/source/config/日志，不能独立重扫历史dist。两版已签名App均已独立重扫。E01仍REVIEW，普通App启动、动态ACL及安装仍NOT_RUN。marker初始控制失败仅保留cause/correction/product_source_changed三字段说明，无原始traceback；修正后的28项指定token控制已核对，不能把初始失败机制称为独立观察。审查器首次review.json的模式/feature位置误判保留，由final-review-v2纠正；不把旧误判当成产品缺陷。6136及更早普通包保留为历史。MemoryVault、同进程备份重开及单个合成PNG不证明真实Keychain、备份跨App重启、全部编解码器/尺寸或完整资产验收；失败可能留下不可达代次。普通App、动态ACL、安装、真实账号、实源、系统/长时及独立WebKit清理仍未验，E01保持REVIEW。

代理测试链使用 `network.rs`共用builder与 `explicit_proxy_tls.py`受控CONNECT/TLS端点。资产测试沿合成备份IPC → 共享恢复body → Publisher恢复资产路径 → MemoryVault；Python oracle独立读取最终PNG，逐失败历史只保留fixture结果。

历史7540草稿IPC产品提交为 `7540c9ec3ff390a96b9dbadd8d340577601b0750`，tree `c98501f300459ba076005becdb78fbe8862bb6ba`。[native-r2回执](../../output/no-account-20261027/draft-ipc/native-r2/summary.json)与[独立复核](../../output/no-account-20261027/draft-ipc/review-native/review.json)记录两版共14个受控fixture进程、115项输入内容图、105项解析器回归及16组实际沙箱负控。Personal直接调用生产 `ad_draft_read`、`ad_draft_save`、`pause_ad`，9成功/9分类拒绝；拒绝要求准确业务错误或对应命令解码错误。Public三个命令均权限拒绝；运行时拒绝不能证明handler不存在，后者单列静态注册/capability证据。

18个有序场景的36份逐步摘要快照来自fixture；Python只对最终磁盘作独立检查，并核对快照链及事件/响应一致性。不把逐步fixture快照称为逐步独立磁盘观察。图片只含合成元数据；没有图片导入/显示、商家发布、ObjectStore或真实Keychain证明。四个跨进程阶段仍属于偏好，不能外推草稿重启/崩溃持久化。native-r1因保留域.example输入被merchant_host拒绝的失败保留，属于fixture输入问题；host与error-class各有真实断言RED/GREEN，历史 `parser/red.log`为模块缺失ImportError，不能作为行为RED。

[本轮Rust r1严格对账](../../output/no-account-20261027/draft-ipc/rust/r1/strict-result.json)为705 passed、1 ignored、3 nested；601个passed名称，按harness清单一致，nested不重复计数。tests/list/fmt/默认Clippy四命令exit0，[两版feature Clippy](../../output/no-account-20261027/draft-ipc/rust/r1/ipc-business-clippy/summary.json)通过；ignored不计入passed，既有专用r7证据单列保留。历史草稿阶段[图谱](../../output/no-account-20261027/draft-ipc/codegraph-status.json)为186文件/5055节点/19898边，complete、pending0。该历史轮次普通包精确绑定6136，7540普通包未构建；E01保持REVIEW，普通App、动态ACL、安装、真实账号、真实Keychain、实源与长时未验，独立WebKit子进程清理仍未证明。下方6136/native-r3及其输入快照差异完整保留。

草稿受控调用链：`draft.mjs` → Tauri IPC → `ipc_business.rs`注册的生产草稿命令 → 本机Publisher。`draft.py`与`test_draft.py`验证有序响应、分类错误及fixture摘要快照；该链不装配远程ObjectStore。

历史6136合成备份IPC源码提交为 `6136fe85fe573e9ebe42316f8e482070e9c34127`，tree `292a93f1013c725bc0579f457eb3cf60039008ee`。此前 `b4d06baa2494d55247d5a41e13a7702cc27c7b23`补足偏好故障与专用r7测试；`3a8161ea0a7ab0acffd21e0ef04cc6de73c3385d`使严格解析器单列ignored。[native-r3回执](../../output/no-account-20261027/encrypted-backup-ipc/native-r3/summary.json)与[独立复核](../../output/no-account-20261027/encrypted-backup-ipc/review-native/review.json)绑定当时的112项输入内容图，不证明完整6136源码构建。其中 `e11_log_privacy.rs`仍为后续Clippy修正前内容，三个components二进制属于生成输入；112项不能统称git源码。该e11修正由后续Rust r2与专用r7验证。89项runner回归包含18项备份解析测试。Personal使用共享恢复body和MemoryVault完成1成功/4拒绝：错误口令、篡改、截断及Vault故障。每次在同一fixture进程重开Publisher检查状态；Public两个备份命令均在权限层拒绝且无写入，不能由此证明运行时handler不存在。

备份失败保护活动状态、旧代次及旧Vault条目；不要求整个目录树不变，可能保留非活动孤立代次。本轮合成备份不含图片资产；四独立进程证据仍只证明选定偏好持久化。native-r1目录初始化失败与native-r2旧Public oracle失败均保留；目录fixture有Rust RED/GREEN，权限oracle有2项RED及18项GREEN。MemoryVault不证明生产原始Keychain命令、备份跨进程恢复、普通App或完整资产覆盖；独立WebKit子进程清理仍未证明。

[Rust r2严格对账](../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/strict-result.json)确认705顶层passed/601个passed名称、1 ignored，合计706记录/602名称；3条nested不重复计数，按harness清单一致。ignored活动记录写拒绝测试需单层Seatbelt，已由[专用r7](../../output/no-account-20261027/ordinary-next/preferences-faults/r7/receipt.json)独立通过。该直接Rust测试覆盖两版：偏好目标阻塞时保留活动记录；活动记录写拒绝时返回错误，已提交偏好可由新状态读回。它描述部分提交行为，不证明双文件事务，也不属于原生IPC。tests/list/fmt/默认Clippy四命令exit0，[两版ipc-business feature Clippy](../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/ipc-business-clippy/summary.json)通过；原粗计数失败回执及此前失败轮次保留，不把ignored计入passed。历史备份阶段[图谱](../../output/no-account-20261027/encrypted-backup-ipc/codegraph-final.json)为183文件/4997节点/19474边，complete、pending0。历史6136普通双包已从6136精确git archive构建；[交付回执](../../output/no-account-20261027/bundles-backup/delivery-summary.json)绑定完整提交 `6136fe85fe573e9ebe42316f8e482070e9c34127`、tree `292a93f1013c725bc0579f457eb3cf60039008ee`及archive SHA256 `ddcf3bc6490e466c163cf290727639b99ee3171abbe93da1d47a717e8d2e4fbe`。每版22项静态检查及2项复制/签名变更完成，各12条目重验通过；备份fixture运行体标记未检出。E01保留REVIEW；[独立包复核](../../output/no-account-20261027/bundles-backup/review/review.json)已通过限定静态验证。Public当前dist已独立重扫；Personal原始dist已被覆盖，只能引用构建时dist证据。普通App启动、动态ACL与安装仍NOT_RUN。旧d0a及更早包保留为历史。普通App、动态ACL、安装、真实账号、真实Keychain、远程发布及系统/长时门槛保持未验。

合成备份调用链：`ipc_business.rs` → `ipc_business/backup.rs` → 共用恢复body → `publisher::restore_backup` → 共享MemoryVault。受控入口覆盖body与Publisher；生产原始命令handler仍未运行。偏好故障专用入口为 `tests/preferences-faults/runner.py`；严格计数入口为 `tests/evidence_reconciliation/reconcile.py`。

历史9f跨进程偏好与共享MemoryVault提取绑定源码提交 `9f0c906776d7ff30adea6d956484d764fd262715`，tree `fc28bbcd4f83ff0d4b50c1349cad1dd6401903e3`。[跨进程原生回执](../../output/no-account-20261027/preferences-persistence/native-r1/summary.json)与[独立复核](../../output/no-account-20261027/preferences-persistence/review-native/review.json)证明两版共4个独立fixture进程：Personal/Public分别写入，再各启新进程读回选定非默认偏好；版本间文件哈希保持，最终4目录0700、4文件0600且单链接。71项runner回归通过，109项构建输入绑定。它只证明受控入口的选定设置跨进程持久化；完整组合、崩溃/断电恢复、同用户恶意文件竞态及普通App仍未验。

本轮Rust r1的tests/list/fmt/Clippy四命令exit0；[Rust r1严格对账](../../output/no-account-20261027/preferences-persistence/rust/r1/strict-result.json)确认704顶层执行/600名称/3嵌套，清单多重集一致，嵌套不重复计数。原粗计数失败回执保留，不与此前r3混为同一轮。历史跨进程阶段[图谱回执](../../output/no-account-20261027/preferences-persistence/codegraph-status.json)为178文件/4887节点/19118边、pending0；下方175/4835/18966为历史快照。该历史轮次普通双包精确绑定d0a2498；该轮未构建9f0c906普通包，普通App启动、动态ACL、安装、真实账号与Keychain保持NOT_RUN。

共享测试支持位于 `apps/desktop/src-tauri/src/publisher/test_support.rs`：MemoryVault供publisher测试与ipc-business受控入口复用。`persistence.py`编排四个独立进程；`business.js`执行偏好写入/读取；`test_persistence.py`覆盖编排与证据拒绝路径。生产Keychain/Vault行为不由该内存实现证明。

历史d0a偏好阶段[双包独立复核](../../output/no-account-20261027/preferences-ipc/review-bundles/review.json)已完成：两版现存App的12条目/7文件、日志及精确归档绑定匹配，二进制/resources运行体扫描复现。Public当前dist可独立重扫；Personal原始dist已被覆盖，仅保留历史dist哈希/扫描回执。不能由二进制扫描重建原始压缩前端输入。E01维持REVIEW，普通App启动、动态ACL与安装仍NOT_RUN。

历史d0a偏好阶段普通双包：[交付回执](../../output/no-account-20261027/bundles-preferences/delivery-summary.json)与[构建回执](../../output/no-account-20261027/bundles-preferences/final-summary.json)精确绑定产品/fixture提交 `d0a2498092f1e6b5cea16b8c30e7b811ead9493b`，不绑定后续文档提交。原构建每版22条静态检查及2条复制/签名变更通过，Public后各12条目重验通过；独立审查已完成限定静态复核。business/limits/偏好快照fixture运行体标记未检出，历史E01保留REVIEW_MARKERS_PRESENT。普通App动态ACL、启动、安装仍NOT_RUN；旧00db包完整保留。

历史d0a偏好IPC源码/fixture提交：`d0a2498092f1e6b5cea16b8c30e7b811ead9493b`，tree：`7fbc4df5d04708c87d6b44486ae46d06825248e3`。两版各22次真实save_preferences调用，5次成功/17次拒绝，各44份前后快照；所有拒绝均保持两个文件不变。106项输入绑定，53项runner解析回归通过。[原生回执](../../output/no-account-20261027/preferences-ipc/native-r1/summary.json)与[独立复核](../../output/no-account-20261027/preferences-ipc/review-native/review.json)限定受控独立入口，不运行普通启动；该历史轮次未验重启后偏好持久化；本轮仅补足上方受控跨进程范围，完整矩阵及普通App仍未验。53项是runner回归，不与证据对账解析器25项混计。

Rust r3四命令exit0；原正则计数失败保留。[仓库严格解析器对账](../../output/no-account-20261027/ordinary-next/rust/r3/strict-reconciliation.json)确认704顶层/600名称/3嵌套，按harness清单一致，嵌套不重复计数；它是r3既有执行的对账，不是额外Rust运行。历史d0a普通双包已从d0a2498精确归档构建并完成原构建静态检查，独立审查已完成限定静态复核；旧00db包及原哈希保留。历史偏好阶段[图谱](../../output/no-account-20261027/preferences-ipc/codegraph-status.json)为175文件/4835节点/18966关系，complete、pending0；旧175/4814/18877为历史快照。

解析器提交为 `ac1d48b49a72df6d83b7e08c05fc6406b3f7e2b8`，tree：`cbe31f21bd652991ada53c26a7507bec382d32d4`。`tests/evidence_reconciliation/`的25项回归及[独立审核](../../output/no-account-20261027/ordinary-next/review-rust-r2/portable-review.json)限定通过；仅支持当前串行libtest/F06三子进程输出格式，不支持任意并发或新嵌套格式。历史回放严格核对704顶层/600名，原失败summary保留；解析器核验证据一致性，不认证回执或重跑Rust。解析器交付阶段的历史普通包绑定00dbf3c，不改标为解析器提交。此前[图谱状态](../../output/no-account-20261027/ordinary-next/codegraph-status-final.json)为175文件/4814节点/18877关系，complete、pending0。

历史00db普通双包独立复核结论为PASS_STATIC_ARTIFACTS_WITH_PROOF_LIMITS：每版12个现存条目/7文件及24份命令日志哈希重新核对，commit/tree/archive均匹配。该历史轮次的临时frontend dist已清理，该次复核不能独立重扫，仅保留原构建扫描回执绑定。普通App动态ACL、启动和安装仍未验；解析器已完成限定独立审核。见[00db历史普通包独立复核](../../output/no-account-20261027/ordinary-next/review-bundles/review.json)。

历史00db普通双包已构建：绑定 `00dbf3c` 的精确归档，每版22条静态检查及2条复制/签名变更通过，Public构建后两版各12条目重验通过。business/limits fixture运行体标记未检出；历史E01权限描述字符串保留REVIEW_MARKERS_PRESENT。普通App动态ACL、启动及安装仍NOT_RUN。独立包复核已完成限定静态核对；解析器已完成限定独立审核。[双包交付回执](../../output/no-account-20261027/bundles-ordinary-next/delivery-summary.json)记录准确路径与哈希。

当前发布命令生产实现仅装配LocalObjectStore，使用本机私有文件；尚无已装配的生产HTTP ObjectStore。广告受控HTTPS客户端及MemoryStore状态机测试不证明发布HTTP CAS已实现或通过；远程对象存储适配与实源验收单列未完成。

历史00db普通包产品源码提交为 `00dbf3c486b65aa5840033954485af2262fb85d3`，tree：`a756309a1e69e6754fd8d40e6ba33770fce226ba`。普通双包已从该提交精确归档构建并完成静态检查；CNC普通包保留为历史已验证包。此前[CodeGraph状态](../../output/no-account-20261027/ordinary-next/codegraph-status.json)为172文件/4748节点/18696关系，complete、pending0，统计不是覆盖率。

新增受控业务入口通过编译特性ipc-business隔离，直接注入合成DesktopState，经WKWebView IPC调用真实activity_log/export_activity；不执行普通DesktopState::new或后台启动。稳定runner r9两版构建及preflight/business限定通过，40回归及实际沙箱负控通过。最终Rust r2四命令exit0；原summary计数失败保留，[独立分组复核](../../output/no-account-20261027/ordinary-next/review-rust-r2/review.json)确认704顶层/600名称，按(binary,name)清单一致，3子进程不重复计数。它与700/596、692/588、CNC684/580分轮记录，原两项原生排除不变。真实账号、普通启动、系统、实源、安装及长时仍未验，详见[独立证据索引](../acceptance/evidence/no-account-20261027/ordinary-next-index.md)。

更新日期：2026-10-27，Asia/Shanghai。项目根目录：`/Users/gua/CodexWorkspaces/claude-switcher`。

## 1. 基线与索引范围

当前开发分支为 `codex/rust-rewrite`。整合基线C49为 `49c2615beac0e1f7ac7352977d3212229fe9b012`，已包含 `codex/gpt-audit-fixes` 的全部代码。整合交付记录为 `84d7bc1b0b0d80630745272361b6aaa41c9259fa`。历史产品修复CNA为 `0297efe2da236427b9a95a584aba53c4f30ce634`。历史产品CNB为 `7851ce10bd00a040bf8c209fd6e763baf63546ff`，tree为 `27d4826b204d079f3a28c48a7f2089d31fe1992e`。CNB补足CLI普通Fix复检和current悬空链接拒绝，保留既有检查、门禁与测试时钟成果。

CNC产品为 `23c9964403e904ab44c2cc334d40992454193221`，tree为 `920c47bf92fc69ce8916266f61327b3766c8fada`。

CodeGraph 1.6.0已增量同步，结果为Already up to date。CNC候选时索引有 **165个文件、4,574个节点、17,868条关系**。历史CNB索引159文件/4,530节点/17,725关系保留为历史；[CNC图谱回执](../../output/no-account-20261027/native-ipc/codegraph-verification.json)绑定当前提交。状态为 `complete`，待同步文件与待解析引用均为零。统计表示自动索引范围，不表示产品测试覆盖率。

仅测试补足提交为 `e4f8cb0c87e27ce0e95758aec5abab57c76d4aa3`，tree：`ef5a8831e7009b3ec9c76098d76a8a75418200e2`；普通产品与App包仍为CNC。四文件仅改测试：`ad_runtime/tests.rs`、`publisher/tests.rs`、`tests/e11_log_privacy.rs`及`updates.rs`测试区。新增8个测试名，无删除；Rust新轮692执行/588名/0失败，list一致，fmt与全部目标Clippy通过，仍排除原有两项原生测试。它不替代CNC原轮684/580、原生六边界及feature1的原证据。[本轮输入复核](../../output/no-account-20261027/branch-audit-next/after-test-commit.json)已核对95个稳定构建输入：92个已提交输入逐项匹配git blob，另3个为既有ignored派生组件。Python7、前端73、TypeScript49及commands/bridge静态读取均重新核对哈希，匹配原成功snapshot；Python200与前端63+2按该范围限定复用，未重跑。

[测试补足图谱回执](../../output/no-account-20261027/branch-audit-next/codegraph-verification.json)记录该轮165文件、4586节点、18090关系，complete、pending0。新测试覆盖G1/G2广告门禁与持久化拒绝、Publisher三槽两期/恢复序号/单侧凭据及G3 wire捕获正控制/4097拒绝；无新产品bug，真实provider与完整35项仍未证明。[本轮机器摘要](../acceptance/evidence/no-account-20261027/branch-audit-followup.json)单列测试证据；33–66小时粗估保留。

根目录 [codegraph.json](../../codegraph.json) 排除 `output/`、`tmp/`、验收证据与审核副本。历史源码和证据原件均保留。`experiments/` 保留索引，但降低查询优先级。这样可避免冻结源码参与当前调用链分析。

自动索引覆盖 Rust、Svelte、TypeScript、JavaScript、Python、Swift 和 Objective-C。Shell、PowerShell、配置和文档仍须按入口单独核对。索引数据库位于本机 `.codegraph/`，不纳入 Git。旧同步记录保留；历史CNB验证见[CNB后续摘要](../acceptance/evidence/no-account-20261027/branch-followup.json)，[CNA/v4摘要](../acceptance/evidence/no-account-20261027/summary.json)保留历史。

## 2. 当前入口与调用链

```text
桌面：src/main.ts → App.svelte／RegionSettings.svelte
                             │ Personal：Operations.svelte
                             ↓ Tauri IPC
       src-tauri/main.rs → commands.rs → bridge.rs／DesktopState
                             │ Personal：publisher_commands.rs → publisher/
                             ↓
CLI：claude-switcher.rs ─→ switcher-core ←─ switcher-guardian.rs
                             │
             ┌───────────────┼────────────────┐
             ↓               ↓                ↓
      检查与账号生命周期   修复与地区设置     广告与更新规则
             │               │                │
             ↓               ↓ 授权 XPC       ↓
      账号档案与事务日志   switcher-helper    本地状态／受信内容
                             ↓ 身份与策略校验
                         固定系统操作

switcher-domain：共享类型与协议 → Rust 模块／生成的 TypeScript 类型
packaging/macos：候选工件 → 安装计划 → 管理员安装／恢复事务
```

`main.rs` 用编译特性固定 Personal/Public，禁止同时启用或同时缺失。Public 不注册 Personal 的运营写入命令。`ipc-smoke`使用独立编译特性，只注册runtime_info、forbidden_probe、finish_smoke三个命令。该旧入口不承载A01载荷。CNC新增独立ipc-limits入口，Personal六边界限定通过，普通业务IPC仍未验。

桌面 `DesktopState` 汇集快照、检查会话、账号动作、修复计划、地区预览和广告刷新。CLI与guardian复用核心业务模块。账号切换通过 `lifecycle::switch_checked`，当前账号启动通过 `lifecycle::launch_checked`。`checks::run_checks` 将前后网络采样和14项检查纳入同一30秒预算。`checks::evaluate_launch` 统一校验报告完整性、有效期和网络代次。`lifecycle` 独立读取当前代次，在账号复核后再次调用统一门禁，再提交系统启动。

账号current读取使用symlink_metadata，悬空符号链接拒绝，不再回退。CLI普通Fix成功apply后执行完整14项检查恰好一次；blocked/error非0且无OK。现代最终JSON带report；legacy带FIX:check和完整report。set-region及GUI语义不变。

广告和更新运行层通过函数指针接收已有 `ClockSample`。生产默认使用 `time::sample`；合成测试不再间接读取真实sysctl。没有新增IPC、环境变量或远程时钟覆盖入口。guardian描述符验证复用原权限谓词，特殊权限位拒绝规则不变。

helper 根据安装记录校验调用身份，再按固定策略执行系统操作。SourceLocal 与 DeveloperId 的更新许可由安装身份和可信发布配置共同决定。Public 的发行标识不能单独证明远端更新可用。

CNC编译期隔离调用链（普通main不运行）：

```text
feature ipc-limits → main → ipc_limits::run → 独立native-ipc/index.html与limits.js
  image invoke → ipc_limits_image → import_ad_image_body → Publisher::import_image/preview
  backup invoke → ipc_limits_backup → restore_publisher_backup_body → 合成closure计数
  preflight → 已注册禁止命令的permission拒绝；Public拒绝Personal载荷命令
```

生产import_ad_image也调用同一import_ad_image_body，顺序仍为限长→MIME→生产publisher。生产备份路径仍进入KeychainVault；fixture替换为无解密closure，不证明生产恢复通过。独立模块不装配DesktopState或后台任务。ipc-smoke与ipc-limits同时启用的互斥编译预期失败已验证。

## 3. 模块与职责

| 路径 | 当前职责 |
| --- | --- |
| `apps/desktop/src-tauri/src/ipc_limits.rs` | 仅ipc-limits特性启用的独立main装配、临时私有Publisher、边界与ACL回执 |
| `packaging/editions/ipc-limits-personal.json`、`ipc-limits-public.json` | 两版测试页/窗口/能力配置；正式包排除fixture |
| `apps/desktop/tests/native-ipc/`、`native-ipc-payload.test.mjs`、`native-ipc-denials.test.mjs` | 独立HTML/limits.js、payload构造及permission分类回归 |
| `apps/desktop/src/` | Svelte 页面、状态呈现、用户动作和生成的类型引用 |
| `apps/desktop/src-tauri/src/main.rs`、`commands.rs`、`bridge.rs` | 应用入口、IPC 注册、桌面状态与核心调用 |
| `apps/desktop/src-tauri/src/publisher/`、`publisher_commands.rs` | Personal 草稿、签名发布、备份、历史整理与素材回收 |
| `apps/desktop/src-tauri/src/update_runtime.rs` | 桌面更新状态、来源约束与更新动作协调 |
| `crates/switcher-domain/src/` | 检查、账号、发行、广告、系统操作等共享类型；导出 TypeScript 类型 |
| `crates/switcher-core/src/checks.rs`、`network.rs`、`diagnostics.rs` | 14 项检查、网络证据和独立代理诊断 |
| `crates/switcher-core/src/accounts.rs`、`migration.rs`、`lifecycle.rs` | A/B 存档、兼容状态、事务恢复、切换与启动门禁 |
| `crates/switcher-core/src/user_repairs.rs`、`regions.rs`、`guardian.rs` | 修复计划、地区变更和用户级守护；复用各自内部辅助模块 |
| `crates/switcher-core/src/ads.rs`、`ad_runtime/`、`updates.rs`、`signed.rs`、`time.rs` | 内容校验、广告运行、软件更新协议、签名和时间连续性；`ad_runtime.rs` 为运行入口 |
| `crates/switcher-core/src/bin/` | CLI、guardian 和 Keychain 元数据探针入口 |
| `crates/switcher-helper/src/` | 授权、XPC、调用策略、系统操作、安装健康与恢复 |
| `packaging/macos/` | 候选包、本地安装、取消、卸载和事务恢复工具 |

依赖方向保持“界面与入口 → 核心 → 共享协议”。特权动作经过 helper 的身份与策略边界。维护时优先复用现有规则，不在界面复制账号、检查或更新决策。

## 4. 测试入口与当前证据

| 入口 | 验证对象 |
| --- | --- |
| `crates/switcher-core/src/` 内测试、`crates/switcher-core/tests/` | 业务规则、事务、检查、诊断与隐私回归 |
| `crates/switcher-helper/src/` 内测试 | 授权策略、系统动作校验与恢复规则 |
| `apps/desktop/src-tauri/src/publisher/*tests.rs`、其他 Rust 内测试 | 发布、备份、整理、回收及运行状态 |
| `apps/desktop/src-tauri/tests/` | 桌面接口相关的核心准备、修复与导出回归 |
| `apps/desktop/tests/`、`apps/desktop/scripts/` | 前端行为、时间转换和既有隔离回归 |
| `packaging/macos/test_*.py` | 假安装、取消、卸载和恢复；使用隔离文件与替身命令 |
| `tests/` | 旧版 Shell 的切换安全与独立代理诊断回归 |

C49历史整合回归保留125个方法、2组VM和44项包静态检查。历史CNA/v4执行Rust671次、567个不同名称，完整保留。历史CNB的v5本轮新执行679次、575个不同名称、0失败，新增8个方法；共享模块重复编入不能计为独立方法。Python200方法、前端60测试及2组VM和类型检查结果复用，未新跑；7个Python输入哈希未变。当前fmt与Clippy通过。原生时钟和已安装XPC连接两项Rust测试单列NOT_RUN；Python三项排除原因见本轮记录。

CNC Rust本轮684执行/580名/0失败，另列feature目录测试1PASS；fmt、全部目标及两版fixture Clippy通过。前端63+2及Svelte0/0按73前端/49TS输入不变复用；Python200本阶段按7输入未变限定复用，未新跑。旧CNB/v5保持历史。Personal/Public隔离preflight及Personal六边界限定通过；备份无解密/Vault。普通CNC App与完整业务GUI/IPC仍为 `NOT_RUN`；普通双包已独立构建，每版22条静态检查与2条复制/签名变更完成；Public构建后两版各12条目重验PASS。路径与哈希见[当前CNC候选绑定](../handoff/无需账号功能进度-2026-10-27.md#0-当前cnc阶段)。新A01标记均无；旧E01权限描述字符串各1处，普通handler无注册/capability无grant，原始扫描保留REVIEW_MARKERS_PRESENT。这只提供静态解释，普通App动态IPC仍为NOT_RUN。实际安装、真实账号、实际外发、平台与长期运行按各自验收记录执行。完整隔离条件未满足时，不启动普通候选试探。后续有限范围审查仅有限推进，未证明35项完整分支覆盖，不报告未经计算的覆盖率。详见[本轮进度](../handoff/无需账号功能进度-2026-10-27.md)和[37项验收](../acceptance/当前候选验收-2026-10-27.md)。

## 5. 历史实现与产品依据

`app/main.swift` 是旧版 SwiftUI 入口，通过已安装的 `claude-switch.sh` 调用检查、切换与修复。`core/`、`daemons/`、`installer/` 保留旧版实现。`windows/`、`linux/` 保留各自平台实现，不能据此宣称当前 Rust 候选已通过跨平台验收。

`experiments/p0/` 是早期验证。审核包、冻结源码和验收证据用于历史复现，不能作为当前产品调用入口。

暖白、墨绿和横向工作台的视觉方向保留。页面、发行差异和交互以[产品 PRD v1.2](../plans/PRD-Rust双版本产品-v1.2.md)为准。[概念图](ui-concept-v1.png)仅提供视觉依据，不证明运行状态。
