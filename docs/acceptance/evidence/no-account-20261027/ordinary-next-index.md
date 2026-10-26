# 本轮独立证据索引：ordinary-next

恢复增量后的[最终Rust回执](../../../../output/no-account-20261027/local-store-recovery-final/rust/r1/final-receipt.json)记录六项检查exit0，严格对账712顶层执行通过/608不同通过名称、1 ignored、15 harness、3 nested；逐harness清单多重集一致，粗统计失败原件保留。两项系统探测按约定过滤，专用单层隔离ignored及既有r7通过仍单列。实际执行基于HEAD093a加未提交publisher/tests.rs，源码snapshot SHA256 `c57ea716ba797b8519827ee54fa91ac9d63bf65bac5c65f4f8d60a7d9f06f3e6`；随后提交2bc6，不是2bc6 archive执行。该轮与下方图片/下载711/607历史轮次分开，普通包仍绑定82fc。

本地存储缺失对象[定向恢复回执](../../../../output/no-account-20261027/local-store-missing-recovery/final-receipt.json)记录3/3顶层通过（1 guard子入口、2父测试），共4分支：提交前/后正常恢复、提交后缺version、提交后缺素材。子进程exit(73)后，一直存活的父进程删除合成对象并重新打开恢复；已提交分支零CAS，仅补写缺失对象，current字节/etag/inode/mtime不变，签名与唯一历史核对。[独立只读复核](../../../../output/no-account-20261027/local-store-missing-recovery/review/review.json)18项证据检查通过；末次仅重新打开Publisher核对持久状态/历史，没有再次打开LocalObjectStore。desktop测试Clippy/fmt通过，未发现产品bug；不是SIGKILL、断电、fsync故障或新启动恢复进程。执行时HEAD093a加尚未提交测试内容；随后测试源码提交 `2bc6bb76d12af9ef3253d354d758613f490817f3`，不能描述为093a或2bc6归档执行。

恢复补测后的当前[图谱](../../../../output/no-account-20261027/final-codegraph/after-recovery.json)为194文件/5219节点/20349边，complete、pending0。先前194/5217/20353保留为历史；缺失版本/素材恢复测试随后提交2bc6，最终全量结果单列如下；图谱不是普通App动态验收。

当前普通双包从精确 `82fc02e1efc20c051cb9ff62ceb32c8f1e9303ec` archive构建，tree `90c25fef7e9e34bbd06ace5cee52bfdec86d6c8e`，archive SHA256 `1cbce8f6242247c886288b16d4ed0e39ddc737611ebc1b1a3b34c1310ac94a6f`。[最终构建回执](../../../../output/no-account-20261027/bundles-image-ipc/final-summary.json)及[静态验证](../../../../output/no-account-20261027/bundles-image-ipc/verification.json)记录两版构建exit0、每版22项静态命令及复制/签名操作；两版dist各自独立保留并重算哈希。fixture运行体标记排除；历史forbidden_probe/finish_smoke权限描述仍REVIEW。只读[独立包复核](../../../../output/no-account-20261027/bundles-image-ipc/independent-review/review.json)通过1640项/0失败，归档逐字节对应82fc；两版各24条检查/操作日志、12条产物及独立dist哈希匹配。签名检查日志只核对，未重跑codesign。首轮tar mode读取器比较错误保留，非产品失败。普通App、动态ACL、安装、真实账号、DeveloperID、公证/Gatekeeper仍未验。下载测试093a与后续测试提交不能改标此包来源，8961及旧包完整保留为历史。

历史图片/下载[最终Rust r1回执](../../../../output/no-account-20261027/image-update-boundaries/rust/r1/final-receipt.json)记录6项检查exit0：tests/list/fmt/默认Clippy及两版feature Clippy。严格对账711顶层passed/607个通过名称、1 ignored、15 harness、3 nested，清单多重集一致；粗统计失败回执保留。两项真实系统采样/服务认证原生测试按约定过滤；专用单层隔离测试ignored，既有r7独立通过单列。运行时HEAD82fc加尚未提交updates内容，结束HEAD仍82fc；随后提交093a，不能描述为093a归档执行。

更新下载测试源码后提交为 `093a87416b20ec2483af9ad86228e3600283e681`；[最终回执](../../../../output/no-account-20261027/update-download-boundaries/final-receipt.json)与[独立复核](../../../../output/no-account-20261027/update-download-boundaries/review/review.json)绑定当时updates工作树内容，执行时HEAD为82fc，不是093a归档构建。10/10 transport测试通过，新增2测试覆盖5个合成响应，沿生产download → fetch → verify路径验证4字节签名工件的完整、声明长度短正文、无长度短正文、分块恰好上限和分块超限。测试不改变生产行为；r1关闭方式/错误层不匹配失败保留，未证明生产接受截断数据。TLS close_notify未独立抓包，分块可能被TLS/reqwest合并；真实大包、外部源及安装未验。

本轮图片IPC代码提交 `82fc02e1efc20c051cb9ff62ceb32c8f1e9303ec`，tree `90c25fef7e9e34bbd06ace5cee52bfdec86d6c8e`。[native-r3回执](../../../../output/no-account-20261027/image-business-ipc/native-r3/summary.json)与[独立复核](../../../../output/no-account-20261027/image-business-ipc/review-native/review.json)记录16个受控fixture进程、18组隔离控制、141项解析器回归及126项输入内容图。Personal通过真实WKWebView IPC调用 `import_ad_image`与 `ad_image_preview`，3成功/5拒绝；Public两个命令权限拒绝，合成根目录为空。

图片r1 CSP失败属于有效夹具回归；模块缺失等初始化错误不能称行为RED。r2因sips隐式临时目录访问失败保留，后用受控ImageIO重跑。覆盖选定1×1 PNG/JPEG及字节边界，不代表完整格式/尺寸/fuzz矩阵；逐步快照来自fixture，最终磁盘独立重算哈希。JPEG像素核对使用已记录的沙箱ImageIO输出，复核者未重跑原生解码。成功预览没有独立前后快照包围，只读结论同时依赖生产实现审查。图片跨进程持久化、普通App、真实账号/Keychain、安装与实源仍未验；WebKit辅助进程整体终止未证明。

历史图片阶段[图谱](../../../../output/no-account-20261027/final-codegraph/status.json)为194文件/5217节点/20353边，complete、pending0。旧8961已验证普通包为历史；当前82fc包构建及复核状态见上方，不能由原生fixture结果代替普通App验收。下方CAS与其他历史执行、包及输入漂移证据保留。

本轮仅测试提交 `dd1d7e37affcca03976acddfe9f4a822d7a19a31`，tree `cb96d94e2c812d4f98371f6749659fe0b2c3eee2`，补足真实LocalObjectStore的受控退出恢复。[定向回执](../../../../output/no-account-20261027/local-store-cas-exit/final-receipt.json)包含2个顶层测试，其中1个为guard子入口；父测试驱动提交前/后两个 `process::exit(73)`切点。子进程退出后，一直存活的父测试进程重新打开并恢复；恢复CAS分别为1次/0次，重获OS锁，核对同一release/revision及签名wire、签名manifest、不可变版本和全部素材，最终恰好1条历史、pending清除，再次重开保持状态。它没有运行Drop；不等同SIGKILL、断电、fsync故障、HTTP CAS或真实Keychain验收。

[全量r1最终回执](../../../../output/no-account-20261027/local-store-cas-exit/rust/r1/final-receipt.json)记录tests/list/fmt/默认Clippy及两版feature Clippy共6命令通过。[严格对账](../../../../output/no-account-20261027/local-store-cas-exit/rust/r1/strict-result.json)为709 passed/605个passed名称、1 ignored、3 nested，按harness清单一致；ignored和nested不重复计入passed。原粗统计失败保留，真实sysctl采样及服务认证探测两项继续跳过；专用r7证据单列，不冒充默认全量执行。历史CAS阶段[图谱](../../../../output/no-account-20261027/local-store-cas-exit/codegraph-status.json)为189文件/5141节点/20172边，complete、pending0。dd1不改变生产源码与普通包绑定：该历史轮次双包归档自8961905；E01 REVIEW及普通App/动态ACL/安装/账号/实源/长时未验边界保留。

本轮资产与代理源码提交：代理 `1836db7f59867bb437c3938610e8d6b603c820a8`；合成PNG资产恢复 `89619059dd473ad04503fa3326476d0526980016`，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`。[代理回执](../../../../output/no-account-20261027/controlled-proxy-tls/final-receipt.json)记录生产client builder等价提取及受控CONNECT/TLS验证。合成CA只在cfg(test)替换信任，不改变生产默认信任；本机受控传输不证明真实代理或账号可用。r1及isolation-red系统编码环境失败保留：macOS在env_clear后生成 `__CF_USER_TEXT_ENCODING`，fixture随后仅放行该编码提示；此失败不称为产品传输缺陷。

[资产native-r2](../../../../output/no-account-20261027/backup-assets-ipc/native-r2/summary.json)与[独立复核](../../../../output/no-account-20261027/backup-assets-ipc/review-native/review.json)证明Personal通过共享恢复body及MemoryVault恢复真实合成1×1 RGBA PNG：1成功/4拒绝、同进程5次重开，最终源/恢复/旧资产的字节、CRC、受限解压、像素、哈希、类型、大小、尺寸及引用独立核对，附加元数据已移除。121项解析器回归包含16项新增资产检查。四个失败场景的不变性来自fixture；只有最终磁盘独立检查。120项历史输入内容图绑定该轮，不能称为完整当前git源码；独立复核时 `network.rs`与 `tests/fixtures/explicit_proxy_tls.py`已有后续漂移。r1虽Cargo exit0，但因构建中输入漂移拒绝二进制归属，失败证据保留。历史parser/red是ImportError，不是行为RED。

[本轮Rust r1严格对账](../../../../output/no-account-20261027/asset-proxy-verification/rust/r1/strict-result.json)确认707 passed/603个passed名称、1 ignored、3 nested，按harness清单一致；四命令exit0，[两版feature Clippy](../../../../output/no-account-20261027/asset-proxy-verification/rust/r1/ipc-business-clippy/summary.json)通过。与先前705轮次分开，ignored及nested不重复计入passed。历史资产代理阶段[图谱](../../../../output/no-account-20261027/asset-proxy-verification/codegraph-status.json)为189文件/5130节点/20104边，complete、pending0。历史8961普通双包已从 `89619059dd473ad04503fa3326476d0526980016` 精确归档构建，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`；[交付回执](../../../../output/no-account-20261027/bundles-asset-proxy/delivery-summary.json)绑定archive SHA256 `7bc9e7dcc4551d25976d53df1ad19d33a5f696c9b558aa87ea52152d8402fa94`。每版22项静态检查及2项复制/adhoc签名操作完成，最终重验通过；fixture运行体标记排除，E01为REVIEW_MARKERS_PRESENT。[最终独立包复核](../../../../output/no-account-20261027/bundles-asset-proxy/review/final-review-v2.json)通过限定静态检查。Public当前临时dist可独立重扫；Personal原dist被Public构建覆盖，仅保留构建时dist回执/source/config/日志，不能独立重扫历史dist。两版已签名App均已独立重扫。E01仍REVIEW，普通App启动、动态ACL及安装仍NOT_RUN。marker初始控制失败仅保留cause/correction/product_source_changed三字段说明，无原始traceback；修正后的28项指定token控制已核对，不能把初始失败机制称为独立观察。审查器首次review.json的模式/feature位置误判保留，由final-review-v2纠正；不把旧误判当成产品缺陷。6136及更早普通包保留为历史。MemoryVault、同进程备份重开及单个合成PNG不证明真实Keychain、备份跨App重启、全部编解码器/尺寸或完整资产验收；失败可能留下不可达代次。普通App、动态ACL、安装、真实账号、实源、系统/长时及独立WebKit清理仍未验，E01保持REVIEW。

历史7540草稿IPC产品提交为 `7540c9ec3ff390a96b9dbadd8d340577601b0750`，tree `c98501f300459ba076005becdb78fbe8862bb6ba`。[native-r2回执](../../../../output/no-account-20261027/draft-ipc/native-r2/summary.json)与[独立复核](../../../../output/no-account-20261027/draft-ipc/review-native/review.json)记录两版共14个受控fixture进程、115项输入内容图、105项解析器回归及16组实际沙箱负控。Personal直接调用生产 `ad_draft_read`、`ad_draft_save`、`pause_ad`，9成功/9分类拒绝；拒绝要求准确业务错误或对应命令解码错误。Public三个命令均权限拒绝；运行时拒绝不能证明handler不存在，后者单列静态注册/capability证据。

18个有序场景的36份逐步摘要快照来自fixture；Python只对最终磁盘作独立检查，并核对快照链及事件/响应一致性。不把逐步fixture快照称为逐步独立磁盘观察。图片只含合成元数据；没有图片导入/显示、商家发布、ObjectStore或真实Keychain证明。四个跨进程阶段仍属于偏好，不能外推草稿重启/崩溃持久化。native-r1因保留域.example输入被merchant_host拒绝的失败保留，属于fixture输入问题；host与error-class各有真实断言RED/GREEN，历史 `parser/red.log`为模块缺失ImportError，不能作为行为RED。

[本轮Rust r1严格对账](../../../../output/no-account-20261027/draft-ipc/rust/r1/strict-result.json)为705 passed、1 ignored、3 nested；601个passed名称，按harness清单一致，nested不重复计数。tests/list/fmt/默认Clippy四命令exit0，[两版feature Clippy](../../../../output/no-account-20261027/draft-ipc/rust/r1/ipc-business-clippy/summary.json)通过；ignored不计入passed，既有专用r7证据单列保留。历史草稿阶段[图谱](../../../../output/no-account-20261027/draft-ipc/codegraph-status.json)为186文件/5055节点/19898边，complete、pending0。该历史轮次普通包精确绑定6136，7540普通包未构建；E01保持REVIEW，普通App、动态ACL、安装、真实账号、真实Keychain、实源与长时未验，独立WebKit子进程清理仍未证明。下方6136/native-r3及其输入快照差异完整保留。

历史6136合成备份IPC源码提交为 `6136fe85fe573e9ebe42316f8e482070e9c34127`，tree `292a93f1013c725bc0579f457eb3cf60039008ee`。此前 `b4d06baa2494d55247d5a41e13a7702cc27c7b23`补足偏好故障与专用r7测试；`3a8161ea0a7ab0acffd21e0ef04cc6de73c3385d`使严格解析器单列ignored。[native-r3回执](../../../../output/no-account-20261027/encrypted-backup-ipc/native-r3/summary.json)与[独立复核](../../../../output/no-account-20261027/encrypted-backup-ipc/review-native/review.json)绑定当时的112项输入内容图，不证明完整6136源码构建。其中 `e11_log_privacy.rs`仍为后续Clippy修正前内容，三个components二进制属于生成输入；112项不能统称git源码。该e11修正由后续Rust r2与专用r7验证。89项runner回归包含18项备份解析测试。Personal使用共享恢复body和MemoryVault完成1成功/4拒绝：错误口令、篡改、截断及Vault故障。每次在同一fixture进程重开Publisher检查状态；Public两个备份命令均在权限层拒绝且无写入，不能由此证明运行时handler不存在。

备份失败保护活动状态、旧代次及旧Vault条目；不要求整个目录树不变，可能保留非活动孤立代次。本轮合成备份不含图片资产；四独立进程证据仍只证明选定偏好持久化。native-r1目录初始化失败与native-r2旧Public oracle失败均保留；目录fixture有Rust RED/GREEN，权限oracle有2项RED及18项GREEN。MemoryVault不证明生产原始Keychain命令、备份跨进程恢复、普通App或完整资产覆盖；独立WebKit子进程清理仍未证明。

[Rust r2严格对账](../../../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/strict-result.json)确认705顶层passed/601个passed名称、1 ignored，合计706记录/602名称；3条nested不重复计数，按harness清单一致。ignored活动记录写拒绝测试需单层Seatbelt，已由[专用r7](../../../../output/no-account-20261027/ordinary-next/preferences-faults/r7/receipt.json)独立通过。该直接Rust测试覆盖两版：偏好目标阻塞时保留活动记录；活动记录写拒绝时返回错误，已提交偏好可由新状态读回。它描述部分提交行为，不证明双文件事务，也不属于原生IPC。tests/list/fmt/默认Clippy四命令exit0，[两版ipc-business feature Clippy](../../../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/ipc-business-clippy/summary.json)通过；原粗计数失败回执及此前失败轮次保留，不把ignored计入passed。历史备份阶段[图谱](../../../../output/no-account-20261027/encrypted-backup-ipc/codegraph-final.json)为183文件/4997节点/19474边，complete、pending0。历史6136普通双包已从6136精确git archive构建；[交付回执](../../../../output/no-account-20261027/bundles-backup/delivery-summary.json)绑定完整提交 `6136fe85fe573e9ebe42316f8e482070e9c34127`、tree `292a93f1013c725bc0579f457eb3cf60039008ee`及archive SHA256 `ddcf3bc6490e466c163cf290727639b99ee3171abbe93da1d47a717e8d2e4fbe`。每版22项静态检查及2项复制/签名变更完成，各12条目重验通过；备份fixture运行体标记未检出。E01保留REVIEW；[独立包复核](../../../../output/no-account-20261027/bundles-backup/review/review.json)已通过限定静态验证。Public当前dist已独立重扫；Personal原始dist已被覆盖，只能引用构建时dist证据。普通App启动、动态ACL与安装仍NOT_RUN。旧d0a及更早包保留为历史。普通App、动态ACL、安装、真实账号、真实Keychain、远程发布及系统/长时门槛保持未验。

历史9f跨进程偏好与共享MemoryVault提取绑定源码提交 `9f0c906776d7ff30adea6d956484d764fd262715`，tree `fc28bbcd4f83ff0d4b50c1349cad1dd6401903e3`。[跨进程原生回执](../../../../output/no-account-20261027/preferences-persistence/native-r1/summary.json)与[独立复核](../../../../output/no-account-20261027/preferences-persistence/review-native/review.json)证明两版共4个独立fixture进程：Personal/Public分别写入，再各启新进程读回选定非默认偏好；版本间文件哈希保持，最终4目录0700、4文件0600且单链接。71项runner回归通过，109项构建输入绑定。它只证明受控入口的选定设置跨进程持久化；完整组合、崩溃/断电恢复、同用户恶意文件竞态及普通App仍未验。

本轮Rust r1的tests/list/fmt/Clippy四命令exit0；[Rust r1严格对账](../../../../output/no-account-20261027/preferences-persistence/rust/r1/strict-result.json)确认704顶层执行/600名称/3嵌套，清单多重集一致，嵌套不重复计数。原粗计数失败回执保留，不与此前r3混为同一轮。历史跨进程阶段[图谱回执](../../../../output/no-account-20261027/preferences-persistence/codegraph-status.json)为178文件/4887节点/19118边、pending0；下方175/4835/18966为历史快照。该历史轮次普通双包精确绑定d0a2498；该轮未构建9f0c906普通包，普通App启动、动态ACL、安装、真实账号与Keychain保持NOT_RUN。

历史d0a偏好阶段[双包独立复核](../../../../output/no-account-20261027/preferences-ipc/review-bundles/review.json)已完成：两版现存App的12条目/7文件、日志及精确归档绑定匹配，二进制/resources运行体扫描复现。Public当前dist可独立重扫；Personal原始dist已被覆盖，仅保留历史dist哈希/扫描回执。不能由二进制扫描重建原始压缩前端输入。E01维持REVIEW，普通App启动、动态ACL与安装仍NOT_RUN。

历史d0a偏好阶段普通双包：[交付回执](../../../../output/no-account-20261027/bundles-preferences/delivery-summary.json)与[构建回执](../../../../output/no-account-20261027/bundles-preferences/final-summary.json)精确绑定产品/fixture提交 `d0a2498092f1e6b5cea16b8c30e7b811ead9493b`，不绑定后续文档提交。原构建每版22条静态检查及2条复制/签名变更通过，Public后各12条目重验通过；独立审查已完成限定静态复核。business/limits/偏好快照fixture运行体标记未检出，历史E01保留REVIEW_MARKERS_PRESENT。普通App动态ACL、启动、安装仍NOT_RUN；旧00db包完整保留。

历史d0a偏好IPC源码/fixture提交：`d0a2498092f1e6b5cea16b8c30e7b811ead9493b`，tree：`7fbc4df5d04708c87d6b44486ae46d06825248e3`。两版各22次真实save_preferences调用，5次成功/17次拒绝，各44份前后快照；所有拒绝均保持两个文件不变。106项输入绑定，53项runner解析回归通过。[原生回执](../../../../output/no-account-20261027/preferences-ipc/native-r1/summary.json)与[独立复核](../../../../output/no-account-20261027/preferences-ipc/review-native/review.json)限定受控独立入口，不运行普通启动；该历史轮次未验重启后偏好持久化；本轮仅补足上方受控跨进程范围，完整矩阵及普通App仍未验。53项是runner回归，不与证据对账解析器25项混计。

Rust r3四命令exit0；原正则计数失败保留。[仓库严格解析器对账](../../../../output/no-account-20261027/ordinary-next/rust/r3/strict-reconciliation.json)确认704顶层/600名称/3嵌套，按harness清单一致，嵌套不重复计数；它是r3既有执行的对账，不是额外Rust运行。历史d0a普通双包已从d0a2498精确归档构建并完成原构建静态检查，独立审查已完成限定静态复核；旧00db包及原哈希保留。历史偏好阶段[图谱](../../../../output/no-account-20261027/preferences-ipc/codegraph-status.json)为175文件/4835节点/18966关系，complete、pending0；旧175/4814/18877为历史快照。

解析器提交为 `ac1d48b49a72df6d83b7e08c05fc6406b3f7e2b8`，tree：`cbe31f21bd652991ada53c26a7507bec382d32d4`。`tests/evidence_reconciliation/`的25项回归及[独立审核](../../../../output/no-account-20261027/ordinary-next/review-rust-r2/portable-review.json)限定通过；仅支持当前串行libtest/F06三子进程输出格式，不支持任意并发或新嵌套格式。历史回放严格核对704顶层/600名，原失败summary保留；解析器核验证据一致性，不认证回执或重跑Rust。解析器交付阶段的历史普通包绑定00dbf3c，不改标为解析器提交。此前[图谱状态](../../../../output/no-account-20261027/ordinary-next/codegraph-status-final.json)为175文件/4814节点/18877关系，complete、pending0。

历史00db普通双包独立复核结论为PASS_STATIC_ARTIFACTS_WITH_PROOF_LIMITS：每版12个现存条目/7文件及24份命令日志哈希重新核对，commit/tree/archive均匹配。该历史轮次的临时frontend dist已清理，该次复核不能独立重扫，仅保留原构建扫描回执绑定。普通App动态ACL、启动和安装仍未验；解析器已完成限定独立审核。见[00db历史普通包独立复核](../../../../output/no-account-20261027/ordinary-next/review-bundles/review.json)。

当前发布命令生产实现仅装配LocalObjectStore，使用本机私有文件；尚无已装配的生产HTTP ObjectStore。广告受控HTTPS客户端及MemoryStore状态机测试不证明发布HTTP CAS已实现或通过；远程对象存储适配与实源验收单列未完成。

日期：2026-10-27，Asia/Shanghai。状态：**进行中**。此索引不改写原37项验收行，不宣称35项完整覆盖。

## 已完成范围

1. 三文件仅测试提交 `30e861aca3c794d521f921d24e1dd112827bddb9`，tree为 `e7c9736977da9fab07ab2f84717bf04378c4a72a`。Rust统一隔离组700次顶层执行、596个不同名称，全部通过；比上一轮新增8名、无删除，不计子进程重复记录。list与执行多重集合一致，fmt、Clippy通过，仍排除两项原生测试。此轮与692/588、CNC684/580分开，后续新增改动未据此宣称全量通过。
2. 独立审查结论为PASS_STATIC_REVIEW，仅覆盖冻结三文件测试diff。生命周期使用Fake，不证明原生账号迁移回滚；广告TLS客户端经受控注入，不证明HttpsFetcher::new或真实源；备份使用合成Vault/Store，恢复门禁清除后未再次重开。不证明普通App、实源或系统行为。
3. Personal/Public均通过受控独立入口，跨原生WKWebView IPC调用真实activity_log和export_activity。合成DesktopState直接注入，未运行DesktopState::new或普通后台启动。历史原始敏感记录正控制存在；预览、导出和再次读取均脱敏，目录0700/文件0600。preflight及五类权限拒绝通过；解析器20项通过。只控制fixture主进程组，独立WebKit辅助进程退出未证明。
4. 显式代理407/502合成端点两项新场景通过。每场景显式代理恰好一个非控制CONNECT，环境/直连sink为零；三个端点各有GET正控制。回执的3个harness条目包括既有分类和无fixture环境时的空操作child guard，不能称3个新场景。未证明成功CA校验TLS隧道、真实代理或完整CLI/IPC。
5. 三个普通切换SIGKILL检查点及全新进程恢复定向回归完成，tests-result退出0，源文件前后哈希一致。资料为合成资料；不证明真实账号登录态、正常App切换、安装或系统恢复。

## 稳定runner r9限定结果

两版fixture构建及preflight/business均通过；独立重放绑定对应执行回执。40项回归通过；106项输入哈希绑定包含源码、配置、图标及既有ignored派生组件，不称106项纯源码。每版构建沙箱均验证credentials、credentials.toml、config、config.toml四个合成文件读取被拒绝。两版运行均实际执行沙箱负控，拒绝私有文件、exec、外网及localhost，同时允许fixture写入。

路径、HOME、链接及profile相关验证保留早期失败尝试，最终40项包含对应负向回归并通过；早期组合失败含helper缺失errors，不作为四类缺陷各自独立有效RED的证明。失败记录为stable-r6/path-boundary-red.stderr、stable-r6/source-link-red.stderr、stable-r7/config-read-red.stderr、stable-r8/receipt-profile-red.stderr；各原始文件及旧轮次保留。r9不把旧PASS迁移至新快照。普通发行二进制已完成运行体标记排除检查；普通启动、安装和系统服务仍未验；只控制fixture主进程组，不证明独立WebKit辅助进程退出。

[稳定runner独立复核](../../../../output/no-account-20261027/ordinary-next/review-stable/review.json)结论为PASS_WITH_SCOPE_LIMITS，无阻断；审查未重新运行测试或App，核对的是既有执行及完整性证据。

## 最终Rust r2与源码冻结

源码提交 `00dbf3c486b65aa5840033954485af2262fb85d3`，tree为 `a756309a1e69e6754fd8d40e6ba33770fce226ba`。tests、list、fmt、Clippy四命令均exit0。原summary保留FAIL_RECORD_COUNT_ACCOUNTING，不覆盖或删除。独立分组复核识别3条嵌套恢复子进程结果，并恢复一条分行结束的父测试；最终704个顶层通过记录、600个名称，按(binary,name)与清单多重集合一致。不能通过固定减3或仅凭Cargo退出码推导该结果。

独立审查仅重放既有日志及完整性绑定，没有新执行测试。解析器已提交并通过25项限定回归及独立审核。两项原生测试继续排除。700/596保留为此前r1轮次，不与r2混计。历史00db普通双包已从本提交完成构建及限定静态检查和独立静态复核。产品提交阶段图谱172文件/4748节点/18696关系，complete，待更新及待解析均0；不表示覆盖率。

## 当前偏好阶段普通双包（独立审查已完成限定静态复核）

精确产品提交 `d0a2498092f1e6b5cea16b8c30e7b811ead9493b`，tree：`7fbc4df5d04708c87d6b44486ae46d06825248e3`，archive SHA256：`cba9d48799b48e1245e18338a2187eae3e8e898d87a8bcd70daafc973927fe74`。原构建静态结果与后续限定独立复核均已记录，不外推为普通App运行通过。

| 版本 | App路径 | 签名主程序SHA256 | 清单SHA256 |
| --- | --- | --- | --- |
| personal | `/Users/gua/CodexWorkspaces/claude-switcher/output/no-account-20261027/bundles-preferences/artifacts/personal/Claude 切换器 Personal.app` | `fe23e0c5d3e910991b745121b72798a077fd5b0b7514dc38113bb4f058743bc2` | `3865ef81e87c5c6c7827c5359b54fb7eeb652716243c18cb7e2b008466cb6aa6` |
| public | `/Users/gua/CodexWorkspaces/claude-switcher/output/no-account-20261027/bundles-preferences/artifacts/public/Claude 切换器 Public.app` | `d4ca8da611d425228e10cf98ece1ed65e83e37da48b3f7db628792e0801089ad` | `dfe0bcf6526cffabf603de5628551a945e6b5d73d3bb5f3de001a2c5b461f22f` |

## 历史00db普通双包静态交付

绑定00dbf3c精确归档，archive SHA256为 `b8cd16b1166acbbcd2dae62ae7cc2e5eb52ddb86db02a766ec50610381f86172`。两版各22条静态检查及2条复制/签名变更完成；Public后每版12条目重验通过。business/limits运行体标记在dist/resources及四个Mach-O二进制未检出。历史E01权限描述forbidden_probe/finish_smoke各1处，保留REVIEW_MARKERS_PRESENT；静态排除不证明普通App动态ACL，后者仍NOT_RUN。独立包复核已完成限定静态核对；解析器已完成限定独立审核，普通启动、安装、真实账号等限制不变。

| 版本 | App路径 | 签名主程序SHA256 | 清单SHA256 |
| --- | --- | --- | --- |
| personal | `/Users/gua/CodexWorkspaces/claude-switcher/output/no-account-20261027/bundles-ordinary-next/artifacts/personal/Claude 切换器 Personal.app` | `5afb4f7dbdc97db933f39904fda6f2e44c6696c46546c9fd76da115a47437812` | `82ec45d704398cfe2ddf57a5a84e65951904833cabb7ec9d2baaa8c3a5b90a3c` |
| public | `/Users/gua/CodexWorkspaces/claude-switcher/output/no-account-20261027/bundles-ordinary-next/artifacts/public/Claude 切换器 Public.app` | `bd4db1a38c9fec0184beb55635ff068a0ac01da84e88486e295ac3dc98e6bd01` | `2f540ca60b72c3a2ed33d082ffad9437aa6ddabcc34e30381d49dc5670e03158` |

## 仍在推进

稳定runner r9已完成下方限定验证；最终Rust统一全量r2已完成四命令及独立分组复核。历史00db普通双包已从00dbf3c精确归档完成构建及限定静态检查。独立包复核已完成限定静态核对；解析器已完成限定独立审核；CNC历史包保留，不把fixture构建视为普通候选包。真实账号、正常App启动、实源、系统、安装和长时验收保持未验。既有33–66小时粗估不因有限推进缩减。

## 原始回执绑定

以下哈希仅绑定本索引读取到的回执快照，最终回执到齐后另行补充。

| 范围 | 原始回执 | SHA256 |
| --- | --- | --- |
| Rust隔离组 | [summary.json](../../../../output/no-account-20261027/ordinary-next/rust/r1/summary.json) | `16003e42063200d8e9be13f61d9687cbefd08ad167f02c17fd2eeaf6e0d381c2` |
| 独立静态审查 | [review.json](../../../../output/no-account-20261027/ordinary-next/review/review.json) | `37754de9b454b6b33020a933d0ea0bbea5430e3528835f508e647d235dff8152` |
| 两版活动记录IPC | [summary.json](../../../../output/no-account-20261027/ordinary-next/business-ipc/summary.json) | `07e9ce864a81784abfc7f6cc353d74242485537fec1411080a59994efdd414f3` |
| 显式代理定向回归 | [report.json](../../../../output/no-account-20261027/ordinary-next/proxy/report.json) | `c63d9e3f3ee9c2f32c1fbec6225558d6fb4abed4914235f5a822ed771bf5ebee` |
| F06进程重启恢复 | [tests-result.json](../../../../output/no-account-20261027/ordinary-next/f06-process-restart/tests-result.json) | `e4bf52e1ede033ba1ce9da217fc13acaa1b5e5b89d215437c6361d46806c56f4` |
| 稳定runner r9 | [summary.json](../../../../output/no-account-20261027/ordinary-next/business-ipc/stable-r9/summary.json) | `cb072f6fc4888908bd1e673edea1518133ba21958cedfe20589fdbb92f638f65` |
| 稳定runner独立复核 | [review.json](../../../../output/no-account-20261027/ordinary-next/review-stable/review.json) | `479079e35093b2238121d89e1f5ab351cd4b92c59b17d5bfb0063de33bc58875` |
| Rust r2计数复核 | [reconciled-summary.json](../../../../output/no-account-20261027/ordinary-next/rust/r2/reconciled-summary.json) | `4f4a6d9d8310b15b74de84c080c578ac545f054283bf3577f2e2ffc9dc97d448` |
| Rust r2独立审查 | [review.json](../../../../output/no-account-20261027/ordinary-next/review-rust-r2/review.json) | `3f19bd173b9fe2c9d6faea0955af4cc4251ac1842d86dbeb3d0c46e2a17830b7` |
| 当前图谱状态 | [codegraph-status.json](../../../../output/no-account-20261027/ordinary-next/codegraph-status.json) | `7b3087a2e245a19e8d8cb21ed717008425c104244b3f15d880242e73ae3acc59` |
| 普通包交付 | [delivery-summary.json](../../../../output/no-account-20261027/bundles-ordinary-next/delivery-summary.json) | `4566703c1b580dd603594716a827ee6cdf8418cd90a83ed537b4dbe7d1b0aa87` |
| 普通包构建 | [final-summary.json](../../../../output/no-account-20261027/bundles-ordinary-next/final-summary.json) | `acac1267781e28d302f15ea0b51c366e4170fc38f5c54783002189f96feca54b` |
| 00db历史普通包独立复核 | [review.json](../../../../output/no-account-20261027/ordinary-next/review-bundles/review.json) | `f36ee5019caad91db144068d798abdc8778515b27aa42a655dff975520c336b7` |
| 解析器限定独立审核 | [portable-review.json](../../../../output/no-account-20261027/ordinary-next/review-rust-r2/portable-review.json) | `aba9b4c07e7e56b701ed7d3691ae390f3d3c052684cd994c87a0ec49bb73f6bb` |
| 最终图谱状态 | [codegraph-status-final.json](../../../../output/no-account-20261027/ordinary-next/codegraph-status-final.json) | `fc781327f16d2518f3ae993b0a6b43dee460cdd5397e30c0f604c7393adf0e0c` |
| 偏好IPC原生结果 | [summary.json](../../../../output/no-account-20261027/preferences-ipc/native-r1/summary.json) | `53c36f46ece0dde5dd361b56488df0c965637eef5e06769b54ad3311d31b18ee` |
| 偏好IPC独立复核 | [review.json](../../../../output/no-account-20261027/preferences-ipc/review-native/review.json) | `b449a9e3c718cf1a9a46ffe20c268e5c7284528bb70131f807765d69e0bb8f8e` |
| Rust r3严格对账 | [strict-reconciliation.json](../../../../output/no-account-20261027/ordinary-next/rust/r3/strict-reconciliation.json) | `0f44da34068d8a2c39a501d906be19355d6b3c3e70b89be210fa66eef8f07a19` |
| 偏好阶段图谱 | [codegraph-status.json](../../../../output/no-account-20261027/preferences-ipc/codegraph-status.json) | `11cea91bd2d34ad5cf9eb645953a33211e7e11e0cd8ddb7772136d6d33743d13` |
| 偏好阶段普通包交付 | [delivery-summary.json](../../../../output/no-account-20261027/bundles-preferences/delivery-summary.json) | `c8b1c44f25ea053fd16a90a5c84c6c937c56f0546de3577ffee8412f3cef90e7` |
| 偏好阶段普通包构建 | [final-summary.json](../../../../output/no-account-20261027/bundles-preferences/final-summary.json) | `ed634761f41b0936bfff13dc799a6b9498bb665faf6a0c14bc260efcfdb0a8bf` |
| 偏好阶段双包独立复核 | [review.json](../../../../output/no-account-20261027/preferences-ipc/review-bundles/review.json) | `3d10265da15bf8867367bdfead149586f969a0ce6eea77db3a7345f3a97d1287` |
| 跨进程偏好原生结果 | [summary.json](../../../../output/no-account-20261027/preferences-persistence/native-r1/summary.json) | `4eaacf23a4f67fd236486da02be0d4d7ee6ff89c36eb5c30ee0fcdf5c8c1c00a` |
| 跨进程偏好独立复核 | [review.json](../../../../output/no-account-20261027/preferences-persistence/review-native/review.json) | `493957c3d2fa724583c40f19e169ace26730d3c0d9e7045e960038c59ebd1fcd` |
| 跨进程阶段Rust严格对账 | [strict-result.json](../../../../output/no-account-20261027/preferences-persistence/rust/r1/strict-result.json) | `f95066c50bedc694b36008ea438fa28d7075ff48bd19e7026fb366fa0ff5b86d` |
| 跨进程阶段原粗计数失败 | [summary.json](../../../../output/no-account-20261027/preferences-persistence/rust/r1/summary.json) | `b2150672996ea1798826b534081de4f619df18515627d5a85e23e88d453870cf` |
| 跨进程阶段图谱 | [codegraph-status.json](../../../../output/no-account-20261027/preferences-persistence/codegraph-status.json) | `66ac16715bc0d52441af110406d2faa5d7060b1856a11b24cec6180e58d20e5d` |
| 备份IPC原生r3 | [summary.json](../../../../output/no-account-20261027/encrypted-backup-ipc/native-r3/summary.json) | `5b9e4c96405949697b73974e8c1c62c223d682afb465093d8944cc2ff76ba337` |
| 备份IPC独立复核 | [review.json](../../../../output/no-account-20261027/encrypted-backup-ipc/review-native/review.json) | `da7cbc5ee634121c58c71dcde0f6e65d5cbe4ed88cbebecd16011b01bc54e269` |
| 备份阶段Rust r2严格对账 | [strict-result.json](../../../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/strict-result.json) | `34e170d66b095ce627518ca0051cc0aaab44388edf484c6ac4af5245809b6fd5` |
| 备份阶段Rust原粗计数失败 | [summary.json](../../../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/summary.json) | `af219482eacf2652ddb8d8f7997e9b3ab7f38f730feef0fcc309ba36886c01a7` |
| 备份阶段两版feature Clippy | [summary.json](../../../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/ipc-business-clippy/summary.json) | `064d2c89f3078d67625e0a74e077b5c02da21298e98928eb1cbf668bb7d0351a` |
| 偏好故障专用r7 | [receipt.json](../../../../output/no-account-20261027/ordinary-next/preferences-faults/r7/receipt.json) | `271ccffc8c478399777085dffc8bef184d9108439e37802f33bc9db77ebd985e` |
| 备份阶段图谱 | [codegraph-final.json](../../../../output/no-account-20261027/encrypted-backup-ipc/codegraph-final.json) | `b7790d7415c77ca371424ee618689ca28d97efb7a9b8bd01605f765aaf584cb0` |
| 目录fixture RED | [result.json](../../../../output/no-account-20261027/encrypted-backup-ipc/path-red/result.json) | `f6d2f0bdd3deada1a12ed093317e19c334aa8097813b4038802ebbcf92f1fd63` |
| 目录fixture GREEN | [result.json](../../../../output/no-account-20261027/encrypted-backup-ipc/path-green/result.json) | `f674cabf7ca99c1e280f5a506879f88e8b0c24dcb593db240ba52f65eb13a2c8` |
| 权限oracle RED | [acl-parser-red.log](../../../../output/no-account-20261027/encrypted-backup-ipc/acl-parser-red.log) | `798c8bac936666816a50b3a45f5679df4a2342c29dd5eb9995ddd3dc4fc13e73` |
| 权限oracle GREEN | [acl-parser-green.log](../../../../output/no-account-20261027/encrypted-backup-ipc/acl-parser-green.log) | `a93cfa7bf7fe0a1507bc3830da553ec02bf6af25c6d91f43ef4927e91e89b690` |
| 6136普通双包交付 | [delivery-summary.json](../../../../output/no-account-20261027/bundles-backup/delivery-summary.json) | `2430a6fd9e1bc96e9bdbd5e5ef13b01a4dbaf1e548971ad8111d8e4b235080c0` |
| 6136普通双包独立复核 | [review.json](../../../../output/no-account-20261027/bundles-backup/review/review.json) | `abf1c1c9ce56818ab583bd8194579ba23f35933d0190393ef48d4eed6b087961` |
| 草稿IPC原生r2 | [summary.json](../../../../output/no-account-20261027/draft-ipc/native-r2/summary.json) | `3fc3bfd2cac95077dd9f5f9da078f9220f16466b09d9db46f698465e48f59ae8` |
| 草稿IPC独立复核 | [review.json](../../../../output/no-account-20261027/draft-ipc/review-native/review.json) | `a7f68b446f0a56e02087cf76e0008391e74b6a8ffdb14108d6c940ef4c07ad54` |
| 草稿阶段Rust严格对账 | [strict-result.json](../../../../output/no-account-20261027/draft-ipc/rust/r1/strict-result.json) | `eab6d7b93e5d79271f0b33784d05a20bc49c7f7e15191810411ef476e1766544` |
| 草稿阶段两版feature Clippy | [summary.json](../../../../output/no-account-20261027/draft-ipc/rust/r1/ipc-business-clippy/summary.json) | `473c2215f51946ef3739403edd7defd696028cebc9d910acf7892c8d33b6badc` |
| 草稿阶段图谱 | [codegraph-status.json](../../../../output/no-account-20261027/draft-ipc/codegraph-status.json) | `0235aa08ecaec842c6813ff66d491660dae8e834b088a41b7dc0677fc2fff6c7` |
| 草稿解析器初始化失败（非行为RED） | [red.log](../../../../output/no-account-20261027/draft-ipc/parser/red.log) | `b0d5a50b2a649da10720f18d04c5503da5c954e5ea9ca3dec3827d999e02994e` |
| 草稿host行为RED | [host-red.log](../../../../output/no-account-20261027/draft-ipc/parser/host-red.log) | `adb9e5a81e4bae61d4ae6b7092bb357a16b241e325d3fbbb652af8685ebe4e11` |
| 草稿host GREEN | [host-all-green.log](../../../../output/no-account-20261027/draft-ipc/parser/host-all-green.log) | `7b63df79dde71f2e1ee4ed045c2b5a6a9859fb1933ddfac9d555373e2e4d334f` |
| 草稿分类行为RED | [error-class-red.log](../../../../output/no-account-20261027/draft-ipc/parser/error-class-red.log) | `3faf77d6cb9779b72463bc2abd1c039d45078131f99c111bac84ae6c54f6e766` |
| 草稿分类GREEN | [error-class-green.log](../../../../output/no-account-20261027/draft-ipc/parser/error-class-green.log) | `603ec2541a6719f3de947704d7240e6d5431a4c69beab55fcf4e169dbd2ebe1f` |
| 资产原生r2 | [summary.json](../../../../output/no-account-20261027/backup-assets-ipc/native-r2/summary.json) | `72c5ca29a5535c9a51bc82d5a61f3671a796b2f2b353650e1f68d34d09bd07a1` |
| 资产独立复核 | [review.json](../../../../output/no-account-20261027/backup-assets-ipc/review-native/review.json) | `7a77d99057862f0d9b96a7996b4514574ecac4477216dc77e1e40499df7341d0` |
| 受控CONNECT TLS | [final-receipt.json](../../../../output/no-account-20261027/controlled-proxy-tls/final-receipt.json) | `bb3c4df253d503d91d7aaa8d2c67036822cc6b66d2a35f80973715b9db7a6d4a` |
| 代理系统编码失败r1 | [receipt.json](../../../../output/no-account-20261027/controlled-proxy-tls/r1/receipt.json) | `2a7bcdb63ab759d2eed92d5bc75b70151e787dc0f237350a6be2db4d214deb3b` |
| 资产代理Rust严格对账 | [strict-result.json](../../../../output/no-account-20261027/asset-proxy-verification/rust/r1/strict-result.json) | `2762bffe7588e1ac25a89aebc9e7634f9ad5c9a9f4721b233c0aa6202f2bc754` |
| 资产代理两版feature Clippy | [summary.json](../../../../output/no-account-20261027/asset-proxy-verification/rust/r1/ipc-business-clippy/summary.json) | `924712534a14c4f8de71e3cbcc9febc2d99f3a75774723c84d0bd70e0ade36b8` |
| 资产代理图谱 | [codegraph-status.json](../../../../output/no-account-20261027/asset-proxy-verification/codegraph-status.json) | `6a0f08a63ece5deeb611d53d7a21ea29acb182a555b570c3882e1899f14692e5` |
| 8961普通双包交付 | [delivery-summary.json](../../../../output/no-account-20261027/bundles-asset-proxy/delivery-summary.json) | `339f09c679ae38c03aa3165c7e9d4dce14c569c3e875799d8269e24dacebc454` |
| 8961普通双包最终独立复核 | [final-review-v2.json](../../../../output/no-account-20261027/bundles-asset-proxy/review/final-review-v2.json) | `35032fae7733835d29a77537f40ed8169d3e07ed3dbe61784947ab8ee080cd21` |
| 8961审查器初始误判（历史） | [review.json](../../../../output/no-account-20261027/bundles-asset-proxy/review/review.json) | `c6b91b90cdaa8b74ff67f767211d75162cce26dbae55668ee8c62b3a467d5074` |
| 本地CAS退出定向回执 | [final-receipt.json](../../../../output/no-account-20261027/local-store-cas-exit/final-receipt.json) | `89cf9d0e04340d30adf6265916b4e083888e039e25d07f093292449f444a9155` |
| 本地CAS全量最终回执 | [final-receipt.json](../../../../output/no-account-20261027/local-store-cas-exit/rust/r1/final-receipt.json) | `188dbdbec04316e75d7b7ab938ef04506c05a8b5117313e1c5600b287eb56078` |
| 本地CAS严格对账 | [strict-result.json](../../../../output/no-account-20261027/local-store-cas-exit/rust/r1/strict-result.json) | `de9eaa5802c0e90a1ce3e0c0c74d79a784233fe48588b91e192dc69bb88f7aa5` |
| 本地CAS图谱 | [codegraph-status.json](../../../../output/no-account-20261027/local-store-cas-exit/codegraph-status.json) | `e4f30ac486624bdf82e3b887f0c0d4bacd36ae2d11c6bb1c14da691512573984` |
| 图片原生r3 | [summary.json](../../../../output/no-account-20261027/image-business-ipc/native-r3/summary.json) | `cf65f33256942d3c2ae7b440c6ad3d4a7162ef7e167e81e96c080b4beaf148e2` |
| 图片原生独立复核 | [review.json](../../../../output/no-account-20261027/image-business-ipc/review-native/review.json) | `87d349f854f63985b5aae8da4706bfeb8a6802bc37e1b01bdc727d1aea5fc561` |
| 历史图片阶段图谱 | [status.json](../../../../output/no-account-20261027/final-codegraph/status.json) | `5eed07f8f517b89425b6cb2dd49a9f270d103056b6f2d6862b8a127a1a8f8576` |
| 更新下载边界最终回执 | [final-receipt.json](../../../../output/no-account-20261027/update-download-boundaries/final-receipt.json) | `86ae64f8146bf453a4ae5ef77ea046fe39a1c58db635bec0e5acb4aedf02d9e1` |
| 更新下载边界独立复核 | [review.json](../../../../output/no-account-20261027/update-download-boundaries/review/review.json) | `405caea789e29400c9d5af5d0d98218c20bf8f25b4c0e4e67c5d8a8baa498bca` |
| 图片下载最终Rust全量 | [final-receipt.json](../../../../output/no-account-20261027/image-update-boundaries/rust/r1/final-receipt.json) | `960ba0690fbf09c8688fd89b74061c1453c634e506ea48a92783e0600527870e` |
| 82fc普通双包构建（独立静态复核通过） | [final-summary.json](../../../../output/no-account-20261027/bundles-image-ipc/final-summary.json) | `a59ea2d0c50b491572926478f6d270a5498faa332fe526d4f3243da9efc7fca5` |
| 82fc普通双包静态验证 | [verification.json](../../../../output/no-account-20261027/bundles-image-ipc/verification.json) | `e850aab0d93954ad98f040b38457b695158da4dcc3ff35e39580c8a0b68ff9d1` |
| 恢复补测后当前图谱 | [after-recovery.json](../../../../output/no-account-20261027/final-codegraph/after-recovery.json) | `52a9c244d70e9fe61178aca485f0ac02f6765418b1e483f47018591068ed7b28` |
| 本地存储缺失对象定向恢复 | [final-receipt.json](../../../../output/no-account-20261027/local-store-missing-recovery/final-receipt.json) | `b9c0dd0345860fe5206b8903e9b5a0b75f7fb4d24ebc643e890b72ce77808212` |
| 82fc普通双包独立只读复核 | [review.json](../../../../output/no-account-20261027/bundles-image-ipc/independent-review/review.json) | `a9c56b160e3eb47252ba7baac6cd388fc9953900cd1b537e39512d652bf7ceb1` |
| 缺失对象恢复独立只读复核 | [review.json](../../../../output/no-account-20261027/local-store-missing-recovery/review/review.json) | `600f9a85e0a98b289658ccf5eedd04edb835e35a5986c183d20de9056f0ba20c` |
| 恢复增量最终Rust六项检查 | [final-receipt.json](../../../../output/no-account-20261027/local-store-recovery-final/rust/r1/final-receipt.json) | `c86737863d2d4e124f820021341e3f0e476ba50f449479730af111ddaa979aae` |

原生排除：`unregistered_or_wrong_signer_service_never_authenticates`、`time::tests::real_macos_continuous_sample_is_available`。
