# Claude 切换器当前代码图谱

当前普通双包[构建回执](../../output/no-account-20261027/bundles-commit-592e1f5-r1/final-summary.json)绑定提交 `592e1f5de74e8b0cf2bde2a25b0e6c11b6da1269`、tree `432e8a55ded9f1e785971fd7e700d32e4dd65744`，archive SHA256 `fa3ded50e06807a3bd800d80b2e5f57dc4a9b901a1fc47c5d7bcf4d9127e4e3f`。每版22项静态检查及两项复制/无时间戳ad-hoc签名操作通过；运行体标记排除PASS，历史字符串保持REVIEW_MARKERS_PRESENT。Personal manifest SHA256 `c401619c3ac3446e17f00c986c54a5ea5e3519cba124e452262ad6e69edfeacc`；Public `ecf3dcb2697d3c4329768124592729551f6784338c4344e2aad20d403031a815`。本轮独立只读审查通过：重新比对git archive SHA、两版各24条命令日志（22检查及2复制/签名操作）、每版7个App文件、Personal5/Public3个dist文件及签名；核对[Personal产物回执](../../output/no-account-20261027/bundles-commit-592e1f5-r1/artifact-personal.json)及[Public产物回执](../../output/no-account-20261027/bundles-commit-592e1f5-r1/artifact-public.json)。审查者未写独立回执，没有重新运行产品；forbidden_probe/finish_smoke字符串仍REVIEW。普通App启动、安装、动态ACL NOT_RUN。c854及更早普通包保留为历史。

原严格沙箱[O_SEARCH修复IPC复验](../../output/no-account-20261027/search-fix-ipc-r1/final-summary.json)绑定提交 `592e1f5de74e8b0cf2bde2a25b0e6c11b6da1269`，15/15 IPC场景、26组负控、172项解析器测试通过；先前失败的Personal business在同一用户目录边界通过。[独立只读复核](../../output/no-account-20261027/search-fix-ipc-r1/independent-review/review.json)PASS，核验23个PID、41项结果及70个日志哈希。该证据仍为feature受控fixture，普通App、真实Keychain和WebKit子进程清理未验证。592e普通包构建及独立复核状态见上方，不能用IPC复验结果替代包验收。

本轮[第三轮Rust最终回执](../../output/no-account-20261027/storage-final-verify/third/final-receipt.json)记录六项门禁exit0，741顶层通过/637不同通过名称、1 ignored、15 harness、3嵌套；两项系统探测按约定过滤。执行HEAD为 `b682e0b398bcfd5e0cbd40ab999655a8cbc20996` 加当时未提交O_SEARCH，源码快照SHA256 `72b173d4fbe3187f7d24eec49ecf7d83702275d425fe89d61aaeeba19a9a8bef`。源码随后提交 `592e1f5de74e8b0cf2bde2a25b0e6c11b6da1269`，不能称为592e归档执行。该轮与下方740/636历史轮次分列；严格IPC现已复验，见上方原沙箱段；592e新包构建及独立只读审查通过，普通App和其他未验门槛不由此通过。

O_SEARCH兼容性修复已有[真实断言RED](../../output/no-account-20261027/storage-directory-race/search-red.log)：0111祖先目录导致PermissionDenied；macOS改用O_SEARCH后[存储23/23 GREEN](../../output/no-account-20261027/storage-directory-race/search-green.log)，覆盖本机当前文件系统的仅搜索权限祖先目录，独立审查已核对该范围。Linux仍使用O_RDONLY，未安装target，编译和对应行为均未验证。原严格IPC复验结果见上方，第三轮全量结果见741/637段；这不构成最终完成结论。

b682e0b真实HOME下测试输出根的[IPC业务尝试](../../output/no-account-20261027/storage-fix-ipc-r1/run-20261027-075444-personal-business-05690f3b/result.json)失败，原因是O_RDONLY打开祖先目录所需权限；不能标产品全流程PASS。首个错误cache路径导致的编译前失败保留。[/private/tmp合成根基线](/private/tmp/claude-switcher-ipc-b682e0b-r1/final-summary.json)绑定b682e0b，15场景/26负控/172解析器通过，SHA256 `74bd539ddfa17d474bef3f288c615e4c2df265e9c48520c31aedec690b9b3144`。该临时目录可能被系统清理，不属于仓库持久证据，本次未复制。该失败随后由592e原严格沙箱复验修复，见上方；合成根基线仍不替代该复验。

历史第二轮[最终Rust重试回执](../../output/no-account-20261027/storage-final-verify/retry/final-receipt.json)记录六项exit0，740顶层通过/636不同通过名称、1 ignored、15 harness、3嵌套子进程；两项系统探测按约定过滤。实际执行HEAD `82f0738fa00b48ae4c3f9ca3076919518b0a8e88` 加未提交工作树，snapshot SHA256 `445a5e60af9b7d45583928fa1ec252fbaa479d74093388830048f4804ce17990`。源码相关提交为9a9cfa6、82f0738、a8706b0、b682e0b；后两项内容执行时尚未提交，不能描述为从后续提交归档执行。首次fmt/Clippy失败仍保留；双版IPC及592e普通包静态审查已完成，普通App和其他未验门槛不由此通过。

本轮首次全量tests/list均exit0，但[fmt日志](../../output/no-account-20261027/storage-final-verify/final/fmt.log)记录exit1：accounts新增断言格式不符；[默认Clippy日志](../../output/no-account-20261027/storage-final-verify/final/clippy.log)记录exit101：storage测试hook的两处thread_local初始化可用const，另有type_complexity。修正后完整重试通过，见本轮最终Rust段；首次失败历史保留，不以driver进程exit0替代各检查结果，也不提前标最终PASS。

本轮账号旧恢复入口对悬空journal的[真实断言RED](../../output/no-account-20261027/recover-dangling/red-retry.log)及[账号40项GREEN](../../output/no-account-20261027/recover-dangling/final.log)单列；首轮red.log仅为沙箱写编译产物权限失败，不是行为RED。[用户修复悬空日志RED](../../output/no-account-20261027/user-repair-dangling/red.log)与[地区恢复RED](../../output/no-account-20261027/user-repair-dangling/region-red.log)各有真实失败，后续[用户修复36项](../../output/no-account-20261027/user-repair-dangling/clean-green-user.log)及[地区27项](../../output/no-account-20261027/user-repair-dangling/clean-green-region.log)通过。地区级测试采用runtime_state=None，不能推广至所有代理状态。证据仅覆盖隔离合成状态，真实账号和服务调度未验。

存储路径竞态[原始RED](../../output/no-account-20261027/storage-directory-race/red.log)实证父目录替换可使读取跟随诱饵、写入诱饵目录；[移动父目录RED](../../output/no-account-20261027/storage-directory-race/moved-red.log)及[系统错误分类RED](../../output/no-account-20261027/storage-directory-race/error-red.log)分别保留。[最终定向22项GREEN](../../output/no-account-20261027/storage-directory-race/error-green.log)覆盖目录锚定、祖先链接拒绝、读写前后身份检查、rename前移位拒绝及临时文件清理、提交后移位/目录同步失败的可能已提交错误，并保留底层系统错误。PostCommitError外层raw_os_error为None，错误链保留内层原因；fsync失败为测试注入。历史O_RDONLY阶段未覆盖祖先目录仅搜索权限；本轮macOS O_SEARCH定向验证见上方，Linux target仍未安装，Linux编译NOT_RUN。该防护有限，不证明所有时刻的目录并发替换均已阻止，也不证明断电或fsync持久性。

当前[O_SEARCH最终图谱](../../output/no-account-20261027/storage-final-verify/codegraph/search-status.json)为199文件/5380节点/21006边，complete、pending0；[先前格式化图谱](../../output/no-account-20261027/storage-final-verify/codegraph/final-status.json)199/5377/20995保留为历史；[格式调整前状态](../../output/no-account-20261027/storage-final-verify/codegraph/status.json)199/5376/20994及此前199/5337/20729均保留为历史。本增量最终完整测试及源码状态见第三轮741/637段，双版IPC及592e包静态审查结果见上方，不套用下方721/617或c854包结论；本历史阶段已验证普通包绑定c854；当前包见上方592e段。普通App、真实账号、Keychain、远程发布和安装仍未验。

本轮[最终Rust r2回执](../../output/no-account-20261027/backup-native-final/rust/r2/final-receipt.json)记录六命令exit0，721顶层通过/617不同通过名称、1 ignored、15 harness、3 nested；两项系统探测按约定过滤。实际执行HEAD `6ce870c152b624bf6133ab2e8a3930e44e805825` 加未提交源码，snapshot SHA256 `71fa103b7d1de949cc25c4de0e97e2dfe212c1b3c0d0803bf03f64878b7022d4`。随后提交ba94e2f（报告路径）、ca64785（journal/迁移）、c854595（备份夹具），不能描述为c854归档执行。历史c854普通双包[构建回执](../../output/no-account-20261027/bundles-commit-c854595-r2/final-summary.json)绑定commit `c854595040332f2818d1a89cca00d1a8de69a7b8`、tree `fbae80caa6d30665bb7dd31d1c7f0e0d3682bef9`、archive SHA256 `e970ed5e8deb13022856f0b06818be64cd1f7ba2c6b394c2fa41fc4f60df7297`。每版22项静态命令及两次复制/签名操作通过，dist分版保留；运行体标记排除通过，二进制原始名称标记仍REVIEW。r1过期fixture标记检查失败保留，r2两项标记迁移回归通过。独立只读复核已核对重做git archive哈希、两版构建日志/主程序组件哈希、两版dist哈希，并实际执行codesign verify有效；[Personal排除回执](../../output/no-account-20261027/bundles-commit-c854595-r2/fixture-exclusion-personal.json)与[Public排除回执](../../output/no-account-20261027/bundles-commit-c854595-r2/fixture-exclusion-public.json)均核对。旧标记误判与两项迁移回归证据保留；forbidden_probe/finish_smoke仍REVIEW。该审查无独立JSON，仅引用原始结果。普通App、安装和动态ACL NOT_RUN。82fc及旧包保留为历史。

当前备份受控原生证据更新为格式化源码重建的r3：[Personal结果](../../output/no-account-20261027/backup-persistence-ipc/native-final-r3/run-20261027-072342-backup-persistence-cfc5f304/result.json)与[Public结果](../../output/no-account-20261027/backup-persistence-ipc/native-final-r3/run-20261027-072351-public-backup-c67d4026/result.json)均通过；Personal PID9465/9500。r2及其只读审查留作历史，不沿用为r3独立审查结论。r3独立只读审查无阻断：131项输入绑定、两进程PID、四失败类别、Public两命令权限拒绝及五组沙箱控制均核对；与r2仅两个Rust源码格式化哈希变动。该审查未生成独立JSON。最终全量及后续源码提交见本轮r2段；真实Keychain和生产备份handler仍未验。

禁网沙箱[备份Python日志](../../output/no-account-20261027/security-backup-final/backup-python.log)记录36项通过。全量r1[失败回执](../../output/no-account-20261027/backup-native-final/rust/r1/failure-receipt.json)记录tests/list/默认Clippy通过、fmt失败；两文件格式化后r2重跑通过，见本轮独立段；原r1失败保留。历史备份阶段[格式化图谱](../../output/no-account-20261027/security-backup-final/codegraph-formatted.json)仍为199文件/5337节点/20729边，complete、pending0；先前codegraph-status.json保留为历史。

本轮安全整改：[检查报告RED](../../output/no-account-20261027/cli-detail/red.log)实证末级symlink覆盖和目录权限错误；复用prepare_data_root及atomic_write后[定向3项GREEN](../../output/no-account-20261027/cli-detail/formatted-green.log)。未证明父目录被并发替换时的安全性。[账号RED](../../output/no-account-20261027/account-journal/red.log)覆盖三类journal悬空及切换守门四项失败；后续[账号38/38](../../output/no-account-20261027/migration-parent/accounts__tests__.log)通过。[migration父目录链接RED](../../output/no-account-20261027/migration-parent/red.log)另有状态漏判；恢复路径原本已拒绝链接，[migration13/13](../../output/no-account-20261027/migration-parent/migration__tests__.log)通过。

账号首次RED未禁网，rustup曾尝试下载，合成HOME内发现工具链目录；来源及下载是否完成无法确认。后续测试固定本机工具链且禁止外网。账号完整证据目录548MiB，不纳入Git；文档仅链接原始证据，不把首次RED描述为全程离线。

历史r2备份跨进程[Personal回执](../../output/no-account-20261027/backup-persistence-ipc/native-final-r2/run-20261027-071614-backup-persistence-6d0e1ffb/result.json)记录两个独立进程：共享body恢复后，由新进程通过生产draft/image读取；[Public回执](../../output/no-account-20261027/backup-persistence-ipc/native-final-r2/run-20261027-071625-public-backup-a8755307/result.json)记录备份permission拒绝、合成根无写。测试使用合成MemoryVault，未验证真实Keychain、生产备份handler或凭据跨进程。独立只读审查核对131项输入哈希前后一致、Personal PID98650/98687、失败矩阵及实际草稿/图片/trust；Public两个backup命令均ACL拒绝且合成根无写，不证明命令未注册。该审查未生成独立review JSON，仅引用上述原始结果。最终完整测试及后续源码提交见本轮r2段；不能由该fixture外推普通App通过。

历史格式化前[图谱](../../output/no-account-20261027/security-backup-final/codegraph-status.json)为199文件/5337节点/20729边，complete、pending0；草稿197/5296/20582保留为历史。

草稿跨进程[native-r3回执](../../output/no-account-20261027/draft-persistence-ipc/native-r3/summary.json)记录129项构建输入绑定、21个独立PID、23组隔离控制、170项解析器测试。Personal通过真实生产IPC保存六条草稿，新进程读取；Public三项权限拒绝且可观察前后目录树不变；再由新进程执行单槽及全暂停，新进程读取全暂停结果。单槽中间值来自即时IPC读取，最终全暂停后独立读取磁盘；不声称全部中间状态跨进程验证。

r1构建尝试保留；r2保存成功后，被oracle对空cleanup的误判阻断。CLI双模块OUT及cleanup两个夹具缺陷各有有效RED→GREEN；未改生产权限或业务。[独立只读审查](../../output/no-account-20261027/draft-persistence-ipc/review-native/final-review.json)631项证据检查无阻断，核对Public精确三项permission拒绝及可观察前后字节一致；这不证明不存在瞬时同字节写，也不证明WebKit子进程全部回收。草稿fixture源码随后提交 `9cbb145532bf4ab6f339a312022949ff65a4ee95`。[本轮最终Rust回执](../../output/no-account-20261027/draft-cross-process-final/rust/r1/final-receipt.json)记录六检查exit0、712顶层通过/608不同通过名称、1 ignored、15 harness、3 nested；逐harness清单多重集一致，粗统计失败原件保留。两项原生系统探测按约定过滤，专用单层隔离ignored与既有r7独立通过仍分列。执行基于HEAD `03fa93c5f596c46fbcc3915aff815a527b32239d` 加未提交草稿fixture，snapshot SHA256 `9b218cfbf0eecdd92a1f9a3aeb5086f432ace5a9db998710dc865b8097126e63`；不是9cbb145归档执行，也不与下方恢复阶段同数值轮次合并。本历史阶段普通包绑定82fc；普通App、真实账号、Keychain、ObjectStore和安装未验。

历史草稿阶段[图谱](../../output/no-account-20261027/final-codegraph/after-draft-persistence.json)为197文件/5296节点/20582边，complete、pending0；194/5219/20349图谱留作恢复阶段历史。

恢复增量后的[最终Rust回执](../../output/no-account-20261027/local-store-recovery-final/rust/r1/final-receipt.json)记录六项检查exit0，严格对账712顶层执行通过/608不同通过名称、1 ignored、15 harness、3 nested；逐harness清单多重集一致，粗统计失败原件保留。两项系统探测按约定过滤，专用单层隔离ignored及既有r7通过仍单列。实际执行基于HEAD093a加未提交publisher/tests.rs，源码snapshot SHA256 `c57ea716ba797b8519827ee54fa91ac9d63bf65bac5c65f4f8d60a7d9f06f3e6`；随后提交2bc6，不是2bc6 archive执行。该轮与下方图片/下载711/607历史轮次分开，本历史阶段普通包绑定82fc。

本地存储缺失对象[定向恢复回执](../../output/no-account-20261027/local-store-missing-recovery/final-receipt.json)记录3/3顶层通过（1 guard子入口、2父测试），共4分支：提交前/后正常恢复、提交后缺version、提交后缺素材。子进程exit(73)后，一直存活的父进程删除合成对象并重新打开恢复；已提交分支零CAS，仅补写缺失对象，current字节/etag/inode/mtime不变，签名与唯一历史核对。[独立只读复核](../../output/no-account-20261027/local-store-missing-recovery/review/review.json)18项证据检查通过；末次仅重新打开Publisher核对持久状态/历史，没有再次打开LocalObjectStore。desktop测试Clippy/fmt通过，未发现产品bug；不是SIGKILL、断电、fsync故障或新启动恢复进程。执行时HEAD093a加尚未提交测试内容；随后测试源码提交 `2bc6bb76d12af9ef3253d354d758613f490817f3`，不能描述为093a或2bc6归档执行。

历史恢复补测后[图谱](../../output/no-account-20261027/final-codegraph/after-recovery.json)为194文件/5219节点/20349边，complete、pending0。先前194/5217/20353保留为历史；缺失版本/素材恢复测试随后提交2bc6，最终全量结果单列如下；图谱不是普通App动态验收。

历史82fc普通双包从精确 `82fc02e1efc20c051cb9ff62ceb32c8f1e9303ec` archive构建，tree `90c25fef7e9e34bbd06ace5cee52bfdec86d6c8e`，archive SHA256 `1cbce8f6242247c886288b16d4ed0e39ddc737611ebc1b1a3b34c1310ac94a6f`。[最终构建回执](../../output/no-account-20261027/bundles-image-ipc/final-summary.json)及[静态验证](../../output/no-account-20261027/bundles-image-ipc/verification.json)记录两版构建exit0、每版22项静态命令及复制/签名操作；两版dist各自独立保留并重算哈希。fixture运行体标记排除；历史forbidden_probe/finish_smoke权限描述仍REVIEW。只读[独立包复核](../../output/no-account-20261027/bundles-image-ipc/independent-review/review.json)通过1640项/0失败，归档逐字节对应82fc；两版各24条检查/操作日志、12条产物及独立dist哈希匹配。签名检查日志只核对，未重跑codesign。首轮tar mode读取器比较错误保留，非产品失败。普通App、动态ACL、安装、真实账号、DeveloperID、公证/Gatekeeper仍未验。下载测试093a与后续测试提交不能改标此包来源，8961及旧包完整保留为历史。

历史图片/下载[最终Rust r1回执](../../output/no-account-20261027/image-update-boundaries/rust/r1/final-receipt.json)记录6项检查exit0：tests/list/fmt/默认Clippy及两版feature Clippy。严格对账711顶层passed/607个通过名称、1 ignored、15 harness、3 nested，清单多重集一致；粗统计失败回执保留。两项真实系统采样/服务认证原生测试按约定过滤；专用单层隔离测试ignored，既有r7独立通过单列。运行时HEAD82fc加尚未提交updates内容，结束HEAD仍82fc；随后提交093a，不能描述为093a归档执行。

更新下载测试源码后提交为 `093a87416b20ec2483af9ad86228e3600283e681`；[最终回执](../../output/no-account-20261027/update-download-boundaries/final-receipt.json)与[独立复核](../../output/no-account-20261027/update-download-boundaries/review/review.json)绑定当时updates工作树内容，执行时HEAD为82fc，不是093a归档构建。10/10 transport测试通过，新增2测试覆盖5个合成响应，沿生产download → fetch → verify路径验证4字节签名工件的完整、声明长度短正文、无长度短正文、分块恰好上限和分块超限。测试不改变生产行为；r1关闭方式/错误层不匹配失败保留，未证明生产接受截断数据。TLS close_notify未独立抓包，分块可能被TLS/reqwest合并；真实大包、外部源及安装未验。

本轮图片IPC代码提交 `82fc02e1efc20c051cb9ff62ceb32c8f1e9303ec`，tree `90c25fef7e9e34bbd06ace5cee52bfdec86d6c8e`。[native-r3回执](../../output/no-account-20261027/image-business-ipc/native-r3/summary.json)与[独立复核](../../output/no-account-20261027/image-business-ipc/review-native/review.json)记录16个受控fixture进程、18组隔离控制、141项解析器回归及126项输入内容图。Personal通过真实WKWebView IPC调用 `import_ad_image`与 `ad_image_preview`，3成功/5拒绝；Public两个命令权限拒绝，合成根目录为空。

图片r1 CSP失败属于有效夹具回归；模块缺失等初始化错误不能称行为RED。r2因sips隐式临时目录访问失败保留，后用受控ImageIO重跑。覆盖选定1×1 PNG/JPEG及字节边界，不代表完整格式/尺寸/fuzz矩阵；逐步快照来自fixture，最终磁盘独立重算哈希。JPEG像素核对使用已记录的沙箱ImageIO输出，复核者未重跑原生解码。成功预览没有独立前后快照包围，只读结论同时依赖生产实现审查。图片跨进程持久化、普通App、真实账号/Keychain、安装与实源仍未验；WebKit辅助进程整体终止未证明。

历史图片阶段[图谱](../../output/no-account-20261027/final-codegraph/status.json)为194文件/5217节点/20353边，complete、pending0。旧8961已验证普通包为历史；历史82fc包构建及复核状态见上方，不能由原生fixture结果代替普通App验收。下方CAS与其他历史执行、包及输入漂移证据保留。

图片受控链：WKWebView → 已注册生产图片命令 → 共用导入body/预览读取 → 合成DesktopState私有文件。验证器独立检查最终文件；ImageIO仅为受控JPEG像素oracle，不代表普通App已启动。

本轮仅测试提交 `dd1d7e37affcca03976acddfe9f4a822d7a19a31`，tree `cb96d94e2c812d4f98371f6749659fe0b2c3eee2`，补足真实LocalObjectStore的受控退出恢复。[定向回执](../../output/no-account-20261027/local-store-cas-exit/final-receipt.json)包含2个顶层测试，其中1个为guard子入口；父测试驱动提交前/后两个 `process::exit(73)`切点。子进程退出后，一直存活的父测试进程重新打开并恢复；恢复CAS分别为1次/0次，重获OS锁，核对同一release/revision及签名wire、签名manifest、不可变版本和全部素材，最终恰好1条历史、pending清除，再次重开保持状态。它没有运行Drop；不等同SIGKILL、断电、fsync故障、HTTP CAS或真实Keychain验收。

[全量r1最终回执](../../output/no-account-20261027/local-store-cas-exit/rust/r1/final-receipt.json)记录tests/list/fmt/默认Clippy及两版feature Clippy共6命令通过。[严格对账](../../output/no-account-20261027/local-store-cas-exit/rust/r1/strict-result.json)为709 passed/605个passed名称、1 ignored、3 nested，按harness清单一致；ignored和nested不重复计入passed。原粗统计失败保留，真实sysctl采样及服务认证探测两项继续跳过；专用r7证据单列，不冒充默认全量执行。历史CAS阶段[图谱](../../output/no-account-20261027/local-store-cas-exit/codegraph-status.json)为189文件/5141节点/20172边，complete、pending0。dd1不改变生产源码与普通包绑定：该历史轮次双包归档自8961905；E01 REVIEW及普通App/动态ACL/安装/账号/实源/长时未验边界保留。

本地CAS测试链位于 `publisher/tests.rs`：父进程 → guard子入口 → 真实LocalObjectStore写前/写后退出 → 存活父测试进程重新打开并恢复Publisher、重获OS锁。该测试链沿本机文件后端运行，不新增生产HTTP ObjectStore。

本轮资产与代理源码提交：代理 `1836db7f59867bb437c3938610e8d6b603c820a8`；合成PNG资产恢复 `89619059dd473ad04503fa3326476d0526980016`，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`。[代理回执](../../output/no-account-20261027/controlled-proxy-tls/final-receipt.json)记录生产client builder等价提取及受控CONNECT/TLS验证。合成CA只在cfg(test)替换信任，不改变生产默认信任；本机受控传输不证明真实代理或账号可用。r1及isolation-red系统编码环境失败保留：macOS在env_clear后生成 `__CF_USER_TEXT_ENCODING`，fixture随后仅放行该编码提示；此失败不称为产品传输缺陷。

[资产native-r2](../../output/no-account-20261027/backup-assets-ipc/native-r2/summary.json)与[独立复核](../../output/no-account-20261027/backup-assets-ipc/review-native/review.json)证明Personal通过共享恢复body及MemoryVault恢复真实合成1×1 RGBA PNG：1成功/4拒绝、同进程5次重开，最终源/恢复/旧资产的字节、CRC、受限解压、像素、哈希、类型、大小、尺寸及引用独立核对，附加元数据已移除。121项解析器回归包含16项新增资产检查。四个失败场景的不变性来自fixture；只有最终磁盘独立检查。120项历史输入内容图绑定该轮，不能称为完整当前git源码；独立复核时 `network.rs`与 `tests/fixtures/explicit_proxy_tls.py`已有后续漂移。r1虽Cargo exit0，但因构建中输入漂移拒绝二进制归属，失败证据保留。历史parser/red是ImportError，不是行为RED。

[本轮Rust r1严格对账](../../output/no-account-20261027/asset-proxy-verification/rust/r1/strict-result.json)确认707 passed/603个passed名称、1 ignored、3 nested，按harness清单一致；四命令exit0，[两版feature Clippy](../../output/no-account-20261027/asset-proxy-verification/rust/r1/ipc-business-clippy/summary.json)通过。与先前705轮次分开，ignored及nested不重复计入passed。历史资产代理阶段[图谱](../../output/no-account-20261027/asset-proxy-verification/codegraph-status.json)为189文件/5130节点/20104边，complete、pending0。历史8961普通双包已从 `89619059dd473ad04503fa3326476d0526980016` 精确归档构建，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`；[交付回执](../../output/no-account-20261027/bundles-asset-proxy/delivery-summary.json)绑定archive SHA256 `7bc9e7dcc4551d25976d53df1ad19d33a5f696c9b558aa87ea52152d8402fa94`。每版22项静态检查及2项复制/adhoc签名操作完成，最终重验通过；fixture运行体标记排除，E01为REVIEW_MARKERS_PRESENT。[最终独立包复核](../../output/no-account-20261027/bundles-asset-proxy/review/final-review-v2.json)通过限定静态检查。Public当前临时dist可独立重扫；Personal原dist被Public构建覆盖，仅保留构建时dist回执/source/config/日志，不能独立重扫历史dist。两版已签名App均已独立重扫。E01仍REVIEW，普通App启动、动态ACL及安装仍NOT_RUN。marker初始控制失败仅保留cause/correction/product_source_changed三字段说明，无原始traceback；修正后的28项指定token控制已核对，不能把初始失败机制称为独立观察。审查器首次review.json的模式/feature位置误判保留，由final-review-v2纠正；不把旧误判当成产品缺陷。6136及更早普通包保留为历史。MemoryVault、同进程备份重开及单个合成PNG不证明真实Keychain、备份跨App重启、全部编解码器/尺寸或完整资产验收；失败可能留下不可达代次。普通App、动态ACL、安装、真实账号、实源、系统/长时及独立WebKit清理仍未验，E01保持REVIEW。

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
