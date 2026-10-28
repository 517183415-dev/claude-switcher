# Claude 切换器 Rust 重写 PRD

## 2026-10-28：整理历史的渠道名称

整理预览的保留和移除记录须显示渠道、序号和发行标识。历史记录不包含独立的存储地点字段时，不得推断为本机发布。两渠道回归与验证边界见[测试记录](../acceptance/evidence/no-account-20261027/history-label-20261028.md)。

## 2026-10-28：远程发布配置的原生接线

Personal 的预览和正式渠道须分别保存并读取发布配置。无效地址须被拒绝，原有效配置须保留。切回本地后，用户须能选回已保留的远程目标。

原生 Swift 状态模型、WebView 桥接和 Rust 命令已用合成密钥库完成隔离验证。[测试及边界](../acceptance/evidence/no-account-20261027/swiftui-remote-settings-20261028.md)。真实 Keychain、真实远程服务、窗口内手动操作和应用进程重启仍待验证。

## 2026-10-28：账号切换的中断恢复

账号切换中断后，新的进程须能依据持久化事务恢复，并保留账号资料和文件属性。恢复重复执行时不得再次改变已完成的结果。

合成资料下，旧式事务新增七个中断切点的新进程恢复测试，均通过。此前三个切点的通过结果单列，没有重跑。[验证范围](../acceptance/evidence/no-account-20261027/f06-fresh-extra-20261028/README.md)。真实账号、断电、普通 App 和完整 F-06 验收仍待验证。

## 2026-10-28：当前代码的隔离验证

对提交 `6631b64b` 集中执行 Rust 工作区与独立 Public 测试。工作区 962 项通过、10 项忽略，另按隔离条件跳过 2 项；Public 196 项通过、2 项忽略。两组测试有重叠，不相加。广告后台失败与 Public 签名接收回归均包含在本轮结果中。[测试命令、日志和边界](../acceptance/evidence/no-account-20261027/final-rust-20261028.md)。

该验证使用合成资料，只允许本机回环网络，不验证真实服务、默认 Keychain、实际安装或长期运行。此前通过的 Swift 与严格 Clippy 检查未重复执行。注册、部署和付款继续暂停。

## 2026-10-28：广告后台更新失败提示

远程广告后台读取或验签失败时，保留仍有效的缓存，并在设置页提示更新失败。条件允许时自动重试；验证成功后清除提示。本机发布内容被篡改时，隐藏该内容并报告校验错误。关闭广告或未确认请求用途时不显示旧的失败提示。广告状态不改变账号和启动检查。

隔离回归使用合成密钥、时钟和内存数据源，覆盖两版的读取失败、签名错误、缓存保留、重建运行时、关闭广告和验证恢复。两版桌面状态接线测试、原有被动快照回归和严格 Clippy 通过。重建运行时不等于真实进程重启；未测试真实网络、云服务和广告提示的窗口展示。[测试日志](../acceptance/evidence/no-account-20261027/ad-status-and-candidate-20261028/green.log)。最新 `afce48da` 产品包已包含本次修复，构建和隔离窗口启动记录见[当前候选验收](../acceptance/当前候选验收-2026-10-27.md)。

## 2026-10-28：此前两版候选包

Personal 和 Public 已从源码提交 `f34f9b9a` 构建，包含广告来源状态和刷新提示修复。两版通过本地 ad-hoc 签名校验。该构建未配置正式广告来源；真实窗口、安装、公证、默认 Keychain 和真实云服务仍待验收。[构建证据](../acceptance/evidence/no-account-20261027/candidate-f34f9b9a-20261028.md)。

## 2026-10-28：广告来源状态和刷新提示

工作区须根据广告运行层实际选择的来源提示配置状态。Personal 优先使用有效本机来源；Public 优先使用有效发行来源。缺少来源与配置错误须分别提示。已选择来源不能视为内容已上线、网络可用或签名验证通过。

刷新未执行时，提示须对应具体原因。关闭广告位、缺少来源、尚未发布、时间状态异常和未到刷新时机不得共用本机发布提示。合成来源与两版状态回归已通过。[测试范围](../acceptance/evidence/no-account-20261027/ad-source-status-20261028.md)。

本机隔离测试已通过桌面命令验证关闭广告位、缺少来源、签名内容刷新和篡改拒绝。独立 Public 特性构建已验证合成本机签名内容的接收、篡改拒绝和恢复。真实远程来源和窗口交互仍待验证。[命令链证据](../acceptance/evidence/no-account-20261027/ad-refresh-dispatch-20261028.md)。

## 2026-10-28：上一轮两版候选包

Personal 和 Public 已从源码提交 `e12ffd43` 构建，包含最新的归档选择与多行脱敏修复；两版通过本地 ad-hoc 签名校验，并在隔离环境中创建窗口。窗口中的具体业务操作、安装、公证、真实账号及真实云服务仍待验收。[构建证据](../acceptance/evidence/no-account-20261027/candidate-e12ffd43-20261028.md)。

## 2026-10-28：活动记录的多行敏感值

活动记录的保存、预览和导出须使用同一脱敏规则。遇到已识别的敏感字段时，隐藏该行和同一条消息中的后续内容，以覆盖多行值及嵌套对象。规则不得宣称能识别所有未标记的敏感数据。

历史记录在读取和导出时脱敏；只读操作不改写历史文件。下一次活动记录成功写入时，保存脱敏后的历史记录。两版隔离回归和既有日志回归通过；脱敏的原生窗口交互和真实历史文件尚未验证。该轮未重建产品包，后续构建见上文。[证据](../acceptance/evidence/no-account-20261027/multiline-redaction-20261028.md)。

## 2026-10-28：备份和归档选择

恢复加密备份或导入旧归档时，取消文件选择或读取失败须保留当前输入的口令，不得启动导入。空文件须提示读取失败。成功读取非空文件后，界面清空输入并交给对应的恢复操作；离开界面仍清空输入。

合成选择结果的回归覆盖取消、读取失败、空文件和成功交接；既有文件类型、符号链接和大小限制测试通过。真实文件选择面板未在本轮测试。[证据和边界](../acceptance/evidence/no-account-20261027/swiftui-archive-picker-20261028.md)。

## 2026-10-28：远程发布的隐私边界

远程发布只传输所需的发布控制信息、指定广告素材和已签名的发布数据。请求不得携带本机账号、Cookie、代理凭据、写入密钥或签名私钥。服务认证需要的访问密钥 ID 可出现在认证头。

本机模拟服务已检查 SDK 发布、匿名回读和接收端验签的请求。[证据及范围](../acceptance/evidence/no-account-20261027/r2-publication-privacy-wire-20261028.md)。真实云端、生产 HTTPS 和完整隐私验收仍待验证。

## 2026-10-28：活动记录并发保护

同一发行版的活动记录读取和写入须共用跨进程锁。在 100 条、60 KiB 的保留限额内，并发操作不得覆盖其他已成功保存的记录。锁长时间被占用时，操作须返回可重试错误，保留已有记录。读取空记录不得创建目录或锁。

两版合成并发、锁占用和不安全锁路径回归已通过。Personal 和 Public 已按修复后的源码重建本地产品包；包未进行窗口或安装验证。[验证记录和边界](../acceptance/evidence/no-account-20261027/activity-concurrency-20261028.md)。

## 2026-10-28：账号与更新操作互斥

账号事务和更新操作必须共用生命周期锁。一个操作持锁时，另一个操作须拒绝写入并保留已有资料。持锁进程意外终止后，后续操作须能够重新取得锁。

两发行的账号协调器与桌面更新运行时已完成跨进程隔离测试。[证据及边界](../acceptance/evidence/no-account-20261027/account-update-contention-20261028.md)。完整 GUI、CLI、XPC 和真实停止路径仍须分别验收。

## 2026-10-28：工作区快照顺序

界面只接受最新的并发快照。通过 `beginAction` 启动操作后，操作前尚未完成的快照不得覆盖本次保存的设置。关闭广告后，旧响应不得重新显示已关闭的广告或图片。

合成传输回归复现并验证了广告设置和快照乱序问题。修复后 Swift 测试 33 项通过；两版产品包重新构建。[证据及验证边界](../acceptance/evidence/no-account-20261027/swiftui-snapshot-order-20261028.md)。

## 2026-10-28：正式渠道备份恢复与远程续发

恢复备份不得自动发布。用户明确续发时，软件须先验证所选目标的当前签名与版本，再补传缺失素材并以条件写入发布更高版本。签名错误必须阻止续发。接收端须验签和校验素材。

本机模拟存储联通测试覆盖上述正式渠道流程。[测试和边界](../acceptance/evidence/no-account-20261027/r2-backup-resume-20261028.md)。测试不证明真实 R2/CDN、默认 Keychain 或线上传播。

## 2026-10-28：工作区操作提示与产品包

广告设置、活动记录导出和商家链接核验开始时，界面须清除前一次操作的成功和错误提示。失败时保留当前错误；重试成功后不得继续显示旧错误。失败的设置保存不得改变显示的已保存设置。

三项新增回归先复现问题；修复后 Swift 测试共 31 项通过。Personal 与 Public 均已重建并通过本地签名和隔离窗口存在检查。该检查不证明实际安装、默认 Keychain、真实账号或远程服务可用；产品包未公证。[验证记录](../acceptance/evidence/no-account-20261027/swiftui-workspace-feedback-20261028.md)。

## 2026-10-28：发布结果提示与备份恢复

每次发布操作开始时清除之前的错误；会产生结果的操作还要清除之前的成功提示。图片、备份或归档读取失败时不得保留之前的成功提示。恢复后的资料刷新失败时，界面必须保持操作锁；重试成功后清除刷新错误并解锁。如果恢复本身也失败，刷新重试不得丢失恢复错误。恢复备份不得自动发布。

隔离 Personal 窗口已通过 Swift 状态模型、WebView 和 Rust 正式处理函数，恢复有效的 64 MiB 加密备份。测试用正常快照和填充字节达到上限，不代表 64 MiB 真实素材；此结果早于后续的恢复错误保留修复。测试没有操作文件选择面板，也没有使用默认 Keychain 或真实远程服务。[验证记录](../acceptance/evidence/no-account-20261027/swiftui-backup-recovery-20261028.md)。

## 2026-10-28：SwiftUI 界面迁移

整体界面使用 SwiftUI 原生玻璃组件。布局沿用现有产品页面，而非效果图布局；采用暖色配色，不使用蓝紫色背景。macOS 26 及以上使用系统玻璃效果，macOS 13–25 使用原生材质兼容。两发行的能力与现有业务、安全门禁保持一致。

总览、账号、环境检查、活动记录、设置与 Personal 广告运营页面已接入 SwiftUI。产品构建脚本启用原生宿主；界面通过隐藏的本地 Tauri 桥调用现有 Rust 业务及权限。直接运行未启用 `native-ui` 的开发构建仍使用原 Svelte 页面。隔离窗口使用合成资料与密钥库；默认 Keychain、真实 Claude、真实 R2/CDN 和 macOS 13–25 实机仍需单独验收。此前 Personal 隔离窗口验证了 64 MiB 无效备份传输及超限拒绝；有效备份的后续验证见上文。该轮两版产品包早于后续修复；最新重建和隔离启动范围见上文。详见 [迁移方案](../design/SwiftUI迁移实施方案-2026-10-27.md)与[该轮验证记录](../acceptance/evidence/no-account-20261027/swiftui-native-20261028.md)。

## 2026-10-27：恢复后的资料刷新

用户确认恢复后，发布及编辑操作须保持锁定，直到页面读到最新资料。读取失败时须显示错误和重试入口；重试成功后才能恢复操作。恢复确认不得自动发布。

产品页面已在隔离窗口覆盖延迟、读取失败与重试，以及取消和明确续发。[证据](../../output/no-account-20261027/native-recovery-ui/README.md)。默认 Keychain 和真实远程服务仍待验。

## 2026-10-27：发布进程中断后的恢复

远程存储已写入但响应丢失时，软件应在重新启动后确认并恢复同一签名版本，完成公开回读后释放锁。不得重复创建版本或提前报告成功。

本机模拟服务已验证写入测试进程终止后的新进程恢复。[证据](../../output/no-account-20261027/r2-process-restart/README.md)。完整桌面 App、默认 Keychain、真实 R2 和线上传播仍待验证。

## 2026-10-27：保留代次损坏时的安全展示

保留代次的状态缺失、损坏或校验失败时，不得展示为已验证历史。目标归属仍显示未知。已用有效、损坏、缺失及恢复后的合成数据验证。[回执与验证边界](../../output/no-account-20261027/storage-final-verify/current/README.md)。

## 2026-10-27：产品页面远程配置

预览与正式渠道的配置须由页面分别保存。切回本机时须保留旧目标；用户可选回保留目标。保存后清空凭据输入框。

隔离 WKWebView 已通过产品 Svelte 页面验证上述操作。测试用合成密钥库，并模拟启动时的环境检查。[证据](../../output/no-account-20261027/native-remote-ui/README.md)。默认 Keychain、进程重启和真实 R2 仍待验；注册、部署和付款暂停。


## 2026-10-27：远程配置的原生 IPC 验证

预览与正式发布渠道必须使用不同存储桶和访问密钥。错误渠道、共用密钥及跨渠道选回目标必须被拒绝；切回本机须保留旧目标。配置响应不得返回凭据。

隔离 WKWebView 经正式 IPC 验证了这些路径，未连接远程服务。[证据](../../output/no-account-20261027/native-remote-config/README.md)。产品页面、默认 Keychain 与真实 R2 仍待验。

## 2026-10-27：远程发布响应丢失的恢复

远程写入结果未知时，不得报告成功或重复生成版本。软件须保留待处理记录和发布锁；确认已提交的内容并完成公开校验后，才可记入历史并释放锁。

本机模拟存储在保存后断连，发布器在同一进程内重新打开后恢复同一签名版本。[证据](../../output/no-account-20261027/r2-uncertain/README.md)。真实 R2、应用进程重启及线上传播仍待验。

## 2026-10-27：旧版测试意图对应关系

旧版 21 项安全和诊断测试，须逐项对应现行 Rust 测试或说明语义差异。[当前映射](../acceptance/evidence/no-account-20261027/f01-intent-map.md)已列出对应测试及历史本机验证范围。历史测试不能代替真实系统验收。

## 2026-10-27：广告失败后的重试间隔

广告读取失败后，从失败时间开始计算 1/5/15/30/60 分钟退避。失败不得延长上次成功时间。结束时钟暂不可用时使用已用单调时间估算；实际休眠场景仍待验证。

本机回归已复现并修复长请求提前重试；相关运行时 38 项通过。[证据](../../output/no-account-20261027/ad-scheduled-recovery/README.md)。真实 R2、后台定时器和 20 分钟线上传播仍待验。注册、部署和付款暂停；SwiftUI 重写仍排最后。

## 2026-10-27：发布内容与接收端组件联通

接收端须使用固定来源，校验签名、版本和图片后采用内容。旧版本或被篡改的内容不得替换已接受版本。发布器生成的两个合成预览版本已通过本机存储和接收端组件联通测试。[证据](../../output/no-account-20261027/r2-recipient-integration/README.md)。

测试没有运行生产 HTTPS 客户端、桌面后台计时器或真实 R2。真实线上传播及 20 分钟门槛仍待验。注册、部署和付款暂停；SwiftUI 重写继续排最后，沿用当前布局和非蓝紫色背景。

## 2026-10-27：远程发布本机联通验证

合成预览目标使用正式配置发布流程和 AWS S3 工具包，向本机模拟对象存储上传图片与两个签名版本。公开回读无认证头；版本号递增，模拟并发写入冲突不覆盖现有公开内容。[证据与范围](../../output/no-account-20261027/r2-sdk-integration/README.md)。

这不代表真实 Cloudflare、TLS/CDN、实际客户端传播、线上防攻击或软件二进制更新已验收。注册、部署和付款保持暂停。SwiftUI 玻璃界面仍排最后，并保留现有布局与非蓝紫色背景。

## 2026-10-27：单槽暂停与历史恢复的原生窗口验证

单槽暂停必须保留其他广告位。全部暂停或历史恢复只生成草稿，须由用户明确发布。历史恢复后的正式版本必须高于已发布版本；不存在的历史不得改变当前资料。

真实 WKWebView 经正式 IPC 验证三槽六期、单槽暂停、全暂停、错误历史拒绝与显式历史恢复发布。13 次响应、8 个检查点验证草稿不自动发布和其他槽保持。四次正式版本经宿主独立 Ed25519 验签，序号逐次递增。[证据](../../output/no-account-20261027/native-pause-history/README.md)。

此测试使用独立前端、合成密钥库与本机对象存储。产品前端、OS 进程重启、默认 Keychain、真实 R2/CDN 和 AD-07 全部边界仍待验。产品代码与依赖未改；UI 重写保持最后，沿用当前布局与非蓝紫色背景。注册、部署、付款暂停。

## 2026-10-27：恢复确认及取消

恢复准备必须生成加密备份。取消、错误摘要或失效会话不得切换当前资料。正确确认后须保留旧代次和备份，且不得自动发布。

真实 WKWebView 通过正式 IPC 验证上述业务；14 次响应和 6 个检查点覆盖拒绝、确认、保留及显式续发。测试使用合成密钥库和独立前端，产品 UI 对话框及默认 Keychain 权限仍待验。[证据](../../output/no-account-20261027/native-recovery/README.md)。R2/CDN 和 AD-10 整项仍未验收。

## 2026-10-27：运营恢复与显式续发的原生窗口证据

备份恢复必须保留旧资料，并进入待确认状态。错误口令不得改变资料。恢复不得自动发布；用户明确续发后，版本必须高于已公开的暂停版本。

上述流程已在真实 WKWebView 经正式 IPC 注册表与 ACL 验证。17 次响应与 5 个宿主检查点覆盖三槽六期、两渠道发布、加密备份、暂停、恢复和更高序号续发。宿主独立核对 Ed25519 签名、素材完整性、旧代次和恢复前后状态。测试使用合成 MemoryVault、本机对象存储及独立测试前端。[证据](../../output/no-account-20261027/native-publisher/README.md)。

相关 IPC 7 项、工具回归 8 项通过；两版测试特性验证和格式通过。复用旧完整测试及原生 ACL 矩阵，未重复执行。默认 Keychain 权限、普通产品前端、原生两次确认、R2/CDN 和 AD-10 整项仍待验。UI 继续排最后；采用参考图的玻璃质感，保留当前布局及非蓝紫色背景。注册、部署、付款暂停。

## 2026-10-27：恢复资料须支持应用进程退出后回读

恢复成功后，新应用进程必须读回恢复代次与密钥，并保留旧代次和旧密钥。恢复待确认状态必须保留；重启不得自动发布。

已用两个顺序启动的独立 OS 进程验证上述要求：写入进程退出并回收后，新进程重新打开显式私有 Keychain 与 Publisher。一个原生用例通过；精确条目及私有库已清理，默认库和搜索列表快照一致。定向编译、Clippy、格式通过；复用上一阶段 236 项完整测试，不重复有效检查。[证据](../../output/no-account-20261027/native-keychain-process/README.md)。范围不含默认库权限、WebView 成功业务、整机重启或 R2/CDN，AD-10 整项仍待验。UI 最后执行，保留当前布局和非蓝紫色背景；注册、部署、付款暂停。

## 2026-10-27：独立系统密钥库的恢复验证

已在两个临时私有 Keychain 内验证正式密钥库类与恢复业务：新条目独占创建、并发仅一方成功、跨库同名条目隔离、错误口令不改当前资料、恢复保留旧密钥和旧代次、同进程重新打开。测试资料清理完毕，受限进程前后默认库和搜索列表快照一致。正式应用默认库选择保留，私有库构造器只编入测试。

一次原生用例通过；阶段末 Personal 236 项通过、该原生用例默认忽略 1 项。Personal 全目标 Clippy、Public 全目标编译、两文件格式通过。前阶段失败不删除；本轮不证明用户默认库权限、OS 进程重启、成功 WebView 或 R2/CDN，AD-10 整项仍未完成。[范围与证据](../../output/no-account-20261027/native-keychain-isolation/README.md)。UI 仍最后执行，保留当前布局和非蓝紫色配色；注册、部署、付款暂停。

## 2026-10-27：原生密钥恢复验收保持待验

系统 Keychain 的新代次创建必须独占；失败不得覆盖旧密钥或改变当前代次。原生验收使用随机合成条目，禁止更改用户默认密钥库与搜索列表。默认测试不访问 Keychain。

已新增直接调用正式 KeychainVault/Publisher 的显式原生测试。本轮首次创建被拒绝；受限进程查询默认 Keychain 返回 -25307。成功恢复、并发独占与同进程重开尚未进入，仍为 WAIT。编译、Clippy、格式及启用保护已核对；合成条目清单为空。[证据](../../output/no-account-20261027/native-keychain/README.md)。此结果不改变既有 MockRuntime 成功证据。UI 最后执行，保留当前布局和非蓝紫色配色；注册、部署、付款暂停。

## 2026-10-27：原生窗口与来源的权限拒绝验证

远程来源的主窗口、未授权的其他窗口，均须拒绝现行版本的全部产品命令。验证须取得实际版本、URL、真实 IPC 桥和完整有序回执；缺桥、缺参或命令不存在不能代替权限拒绝。

两版原生 WKWebView 已完成这两个上下文：Public 每处 23 条、Personal 每处 48 条，共 142 条明确拒绝。测试采用普通生产入口及实际 ACL，前端和窗口为受控测试配置；不升级原产品前端、成功业务、Keychain、实源和系统验收。[原始证据、两项测试工具回归与范围](../../output/no-account-20261027/native-window-unlocked/README.md)。SwiftUI 玻璃效果重写仍排最后，保留当前布局及非蓝紫色背景；注册、部署、付款暂停。

## 2026-10-27：备份恢复和保留资料的成功 IPC 路径

密钥由应用统一持有的 Vault 边界管理，正式版本默认使用系统 Keychain。生成、发布、配置、清理和恢复共用同一边界；Public 不注册此服务，前端不能选择合成密钥库。锁异常须失败关闭。

普通产品 IPC 已补合成密钥库成功验证：导出加密备份、恢复旧备份后保留旧代次与旧密钥、明确发布时核对正式最高序号并续发；恢复准备须能取消，确认后保留原资料。保留资料可加密导出，另一隔离安装可导入，导入不得改变当前 active/state 或自动发布。重复密钥生成和已有代次覆盖须拒绝。

阶段末 Personal 主程序 236 项、Public 主程序 35 项通过；两版四个集成目标各 140 次通过，同一专用 Seatbelt 用例各按原配置忽略一次，不计为通过。两版 all-target Clippy 和六个变更文件格式检查通过，保留三条既有 Personal 迁移风格警告及 ts-rs 提示。294 个源码/构建/测试输入在完整验证后无漂移。CodeGraph complete、pending 0：241 文件、6373 节点、26801 条边。

[阶段证据与验证范围](../../output/no-account-20261027/publisher-vault-dispatch/README.md)分别记录 MockRuntime/MemoryVault 和待验的真实 Keychain、原生 WebView、R2/CDN 传播。只增加本机成功路径证据，不升级系统、实源或长期门槛。UI 最后实施，只参考玻璃效果，保留当前布局及非蓝紫色配色；注册、部署、付款暂停。

## 2026-10-27：拒绝恢复时保护当前资料

恢复备份和导入归档拒绝错误口令、损坏容器或无效快照后，当前发布代次、草稿、素材及目录权限须保持不变。新增生产 handler 的 MockRuntime 测试验证八类拒绝和状态重建回读；[结果与限制](../../output/no-account-20261027/recovery-dispatch/README.md)。该阶段只覆盖拒绝；成功路径见文首后续证据，原生 Keychain 仍待独立验收。

## 2026-10-27：恢复后的发布提示

恢复备份只恢复本地资料，反馈必须明确“尚未发布”。用户核对草稿后明确发布，软件核对所选发布目标的最高序号并补齐素材。未完成发布的提示同样适用于本机与 R2，不能固定描述为本机结果。[两项缺陷回归与验证边界](../../output/no-account-20261027/publication-feedback/README.md)。

## 2026-10-27：普通产品与测试共用 IPC 注册表

新增 `src-tauri/src/ipc.rs`：公共命令只声明一次，Personal 运营命令只声明一次。普通 main 与 MockRuntime 共用 `ipc::handler`，移除六命令测试路由及权限测试对 main 文本的脆弱解析。迁移前后有序注册保持 Public 23 / Personal 48；现行 ACL 独立核对一致。两个 AppHandle 命令及共用退出函数改为泛型 Runtime，业务、成功条件和退出时序不变。

新增两项测试在两版执行：全部命令的其他窗口及远程来源拒绝、Public 全部运营命令拒绝、选定缺参拒绝后目录无业务文件；非默认偏好保存、无效偏好拒绝且持久字节不变、合成敏感活动读取/导出一致、0600 文件及 App 状态重建回读。Personal 既有三槽六期草稿闭环也使用完整产品分发表。此处仍是 MockRuntime，不代替 WKWebView 或所有业务分支。

阶段末 Personal 230 项、Public 35 项全部通过；两版 Clippy 与格式检查通过，Personal 保留三条既有迁移风格警告。首轮 Public 测试辅助方法条件编译失败保留，修复后只重试失败版，再执行一次集中完整集合。[计划](../plans/普通IPC完整注册表-实施计划-2026-10-27.md)与[证据](../../output/no-account-20261027/ordinary-registry/README.md)。CodeGraph complete、pending 0：240 文件、6345 节点、26455 条边。

本轮确认 Mac 仍锁屏，没有重复原生窗口尝试。完整动态窗口矩阵、原生 Keychain、系统与实源验收仍待完成。SwiftUI 玻璃 UI 最后执行，保留当前布局；注册、部署、付款暂停。

## 2026-10-27：排期日期拒绝无效日历输入

修复生产 `operations-date.ts` 的日期归一化缺口：无效闰日或不存在的日期曾被 JavaScript 改为其他日期，非 datetime-local 语法也能进入转换。新增输入语法与日历一致性校验，复用既有 invalid 拒绝分支；保留现有 DST 缺失/重复时刻规则。仅解析函数复现，不外推原生日期控件行为。

回归先取得 7 通过/2 失败，再定向 9 项通过。阶段末前端 67 项通过、既有核心准备脚本通过，类型检查 0 错误/0 警告，两版前端构建通过；未重跑 Rust。原始日志和源码绑定见[阶段证据](../../output/no-account-20261027/campaign-date-validation/README.md)。CodeGraph complete、pending 0：239 文件、6337 节点、26423 条边。

真实日期控件、普通窗口动态矩阵、系统与实源门槛仍待验。SwiftUI 流体玻璃 UI 排最后，保留当前布局；注册、部署、付款保持暂停。

## 2026-10-27：普通命令解析与草稿业务闭环

新增 `ipc_dispatch_tests.rs`，在 Tauri MockRuntime 中使用真实 InvokeRequest 参数解析、现行普通 ACL 和六个生产 publisher 处理函数。覆盖合成图片导入/预览、三槽六期草稿、逐槽修改与暂停、全部暂停、重建 App 状态回读，以及缺参、排期重叠、其他窗口和损坏 state 的拒绝；独立核对磁盘草稿及拒绝后的原始字节。

复用既有权限解析器与隔离 DesktopState 构造器，只为测试启用已有 Tauri 的空 `test` 功能，不新增依赖库或修改 Cargo.lock。两个新测试定向通过；阶段末桌面后端 228 项通过、0 失败，Clippy 和格式检查通过，保留三条既有迁移风格警告。完整套件包含新测试，不与首次定向次数相加。 Public 测试目标编译检查通过，仅代表编译。阶段末检查见[最终回执](../../output/no-account-20261027/ipc-dispatch/final-checks.json)，详细范围见[证据说明](../../output/no-account-20261027/ipc-dispatch/README.md)。CodeGraph complete、pending 0：239 文件、6337 节点、26414 条边。

真实 WKWebView 动态矩阵仍待验：本轮确认 Mac 仍锁屏，未重复窗口尝试。模拟路由仅包含六个命令，不能代替完整产品注册表或原生 Keychain/真实存储验收。UI 重写排在任务列表最后：采用 SwiftUI 流体玻璃质感，保留当前布局、导航与功能位置，不照搬参考图布局，不使用蓝紫色背景；注册、部署、付款继续暂停。

## 2026-10-27：F-05 跨卷复制与容量失败的局部验收

F-05 生产路径现取得独立 APFS 卷两个方向的往返证据。真实 ENOSPC 压力下，归档 A 与恢复 B 的 32 MiB 复制均失败；active、A/B 存档、已提交 state/current 和清理状态保持原值。独立核对内容、权限、owner/group、mtime 与 xattr。

有效结果为 `r3` 三项与 `r6` 一项，按源码片段及哈希复用，不称为一次全绿运行。运行器两项单元测试、最终 Clippy 与格式检查通过。失败夹具记录、源码绑定和卸载证明见[阶段证据](../../output/no-account-20261027/f05-volume-copy/README.md)。CodeGraph complete、pending 0：238 文件、6321 节点、26370 条边。

完整 F-05 分支、普通 App 动态 IPC 及其他系统验收仍待完成。下一步继续前序核心/IPC 缺口；UI 重写仍最后执行，沿用当前布局。注册、部署、付款继续暂停。

## 2026-10-27：账号数据保持的局部原生证据

F-05 新增两个生产事务路径测试：真实 APFS 上 A/B 最新嵌套内容、权限、时间戳、xattr 和复制身份保持；Claude 仍运行时不改变资料、状态或 current。仅进程观察被注入。没有存档的 B 按现有产品合同允许空白资料，不能使用旧测试专用路径的 NotFound 行为作为验收标准。

本阶段核心测试取得 451 个不同顶层用例的通过结果：首次 441 项及修正运行环境后失败项的 10 次单项重跑；不是一次全绿运行。Clippy、新测试格式检查及源码忽略规则回归通过。[范围与失败记录](../../output/no-account-20261027/f05-native-metadata/README.md)。不升级真实账号、跨卷、磁盘耗尽、断电或完整 F-05 验收。UI 重写仍排最后并沿用当前布局，注册、部署、付款保持暂停。

## 2026-10-27：远程来源与其他窗口测试工具

已补测试窗口与本机服务器，保留普通 Rust 后端和实际权限。Public/Personal 计划逐上下文拒绝 23/48 个命令，校验完整序列、版本、URL 和真实 IPC 桥；缺桥、缺参或未知命令不能计 ACL 通过。复用同一报告读取与隔离生命周期，不新增产品测试命令。

12 项 Python 测试、9 项 Node 测试通过。修复测试客户端继承代理导致本机请求 502，并补失效代理、异常关闭及端口复用回归。Public 构建成功，两次运行未取得回执；系统随后确认 Mac 锁屏。两次直接进程均已停止。动态状态仍待验，Personal 未启动边界构建；不能归因所有历史失败或升级完整矩阵验收。[证据](../../output/no-account-20261027/ordinary-ipc-boundaries/README.md)。

产品 UI、Rust 业务、权限和系统配置未改；注册、部署、付款继续暂停。原生 SwiftUI 玻璃效果重写仍排最后，布局保留当前产品布局。


## 2026-10-27：普通后端真实 WebView IPC

新增 `tests/native-ipc-policy`：普通 Rust main、命令处理函数、两版 capability/CSP → 测试 frontendDist → 真实 WKWebView IPC → 宿主读取页面 JSON → 独立核对合成 HOME 文件。不添加产品命令或 fixture feature；测试 App 包装与测试前端均单独标明。

两版共 14 次业务成功、21 次缺参拒绝、33 次 ACL 拒绝（含 Public 全部 25 个运营命令）。共 68 条响应，不作为 68 个独立测试名。7 项 Python 校验测试和 1 项 Node 报告回归通过。修复夹具中间目录 755 问题并补回归。[阶段证据](../../output/no-account-20261027/ordinary-ipc-runtime/README.md)保留未成功尝试、有效回执和源码绑定。

远程来源、其他窗口及全部业务成功路径的动态矩阵仍未完成；不升级为完整普通 App 验收。CodeGraph complete、pending 0：232 文件、6164 节点、25338 条边。产品 UI 布局未改，原生 SwiftUI 玻璃效果重写仍排最后。注册、部署、付款保持暂停。

## 2026-10-27：普通发行 IPC 权限策略证据

普通 Personal/Public 的 capability 必须与命令注册表一致；授权限定本地 main 窗口。Public 不授予 Personal 的 25 个运营命令。新增测试直接使用 Tauri 权限解析器检查实际配置与生成 manifest，不执行命令处理函数。

2 个测试名在两版执行共 4 次通过；Personal 48、Public 23 个授权匹配。两版 Clippy 与格式检查通过，保留既有警告；只修正导入顺序后复用有效测试。[阶段回执](../../output/no-account-20261027/ordinary-ipc-policy/receipt.json)不代表完整普通 App 动态 IPC、真实 Keychain/R2 或远程传播通过。

UI 重写仍排在最后。保留当前 UI 布局、导航与功能位置；效果图只作为原生 SwiftUI 玻璃材质参考，背景继续遵守非蓝紫色要求。注册、部署、付款暂停。

## 2026-10-27：远程发布校验与说明一致性

R2 公开来源必须使用 HTTPS 域名；IPv4 和 IPv6 字面量均拒绝。修复遗漏 IPv6 回环地址的问题，新增三个 IPv6 回归输入。配置保存、目标身份和传输打开共用同一校验。

发布页按真实行为说明：配置 R2 时软件自动上传，未配置时使用本机目录。软件执行版本核对和签名/素材回读；远程正式内容由固定对应域名、公钥的客户端自动检查并验签。历史记录按预览/正式渠道显示，不再误称全部在本机发布。布局保持现状。

[本阶段证据](../../output/no-account-20261027/r2-origin-ip-literals/README.md)保留两个缺陷的失败复现与修复检查。真实 R2、Keychain、远程客户端传播及完整普通 App 动态 ACL 仍未证明；注册、部署、付款暂停。SwiftUI 重写仍排在最后。

阶段末检查：桌面后端 224 项、前端 64 项及两组 VM 回归通过；Svelte 检查 0 错误/0 警告，两版前端构建、Public 编译和格式检查通过。Clippy exit 0，保留 3 条既有迁移风格警告。R2 定向 11 项已包含在完整套件内，不累加。

## 2026-10-27：后置 SwiftUI 流体玻璃 UI 重写

用户已确认：使用原生 SwiftUI 组件重写 UI，流体玻璃只作为视觉效果参考。页面布局、导航结构、内容分区及功能位置沿用当前产品 UI，不采用效果图布局。配色保持非蓝紫色，参考暖白、浅灰和鼠尾草绿版本。

该任务排在当前工作清单最后，先完成前序功能与验证。Personal/Public 差异、安全门禁和 Rust 核心业务继续保留。现有 Tauri/Svelte 实现尚未迁移，不能把概念图视为已完成界面。实施边界与参考图见[设计约束](../design/SwiftUI流体玻璃重写-排期与设计约束-2026-10-27.md)。注册、部署和付款继续暂停。

## 2026-10-27：普通 App 有限业务验证

从源码提交 `9d6a3efcfc6e907ae7797633a843720533a94b34` 的精确归档构建普通 Personal/Public App。两个发行均未启用测试 feature。通过实际界面验证广告偏好保存、单独关闭、进程重启持久化、活动记录读取与脱敏导出；独立读取合成目录核对导出内容与导出时记录一致，相关文件为 0600。

Personal 另验证运营草稿保存及重启后序号与开关保持、已保存本机来源读取和 R2 配置表单。没有填写或保存真实 R2 凭据，没有创建真实签名密钥。Public 没有运营导航；这不证明全部越权命令的动态拒绝。Public 验证期间 Personal 的已记录文件摘要不变。

运行使用独立 HOME 与 Seatbelt。两版负控分别确认外网连接、真实 Library 读取、security 和 sudo 执行被拒绝，合成目录写入成功。14 项环境检查显示 0 通过、1 提示、13 阻塞；不算启动验收通过。真实 Keychain、安全服务连接负控、系统托管 WebKit 回收、安装、远程 R2 与传播仍未验证。

[普通 App 回执](../../output/no-account-20261027/ordinary-app-r1/receipt.json)保存构建、实际 UI、重启和独立磁盘检查的范围。每版 22 项静态检查通过；运行体夹具标记排除通过，二进制名称标记仍为 REVIEW。两版构建包含前端检查与构建。产品源码未变，复用上一阶段有效后端结果，没有重跑或累加。SwiftUI UI 重写仍为最后一个排队任务。

## 历史阶段：远程发布锁与多目标清理（本地验证阶段）

Personal 的远程发布先取得目标锁。整理与文件回收先核对全部目标配置、目录身份和当前内容，再持有全部目标锁执行原有确认流程。旧目标缺少访问资料、锁被其他发布器持有、签名无效或版本回退时停止操作。公开域名不同但桶和渠道相同的目标共用一把物理锁。

锁不自动过期，不抢占其他发布器。请求结果不明时，软件在本机保留所有者意图；重试先用新的条件写入隔离旧请求。有待回查发布时保留锁；恢复备份或切换有效数据代之前须解决意图。锁意图不含写入密钥，不随可移植备份复制；丢失意图时不能自动接管原锁。

索引整理和文件回收仍是两次独立确认。软件验证全部目标的签名 current，并保留旧目标当前内容、历史、待发布和恢复证据引用的素材。回收前重新读取引用。操作失败或只取得部分锁时释放已取得的无待恢复锁；无法确认释放时保留意图并报告失败。

同桶更换域名曾复用旧发布 ID，造成不可变版本文件冲突。现在新操作避开全部目标历史已使用的同渠道 ID；已有待恢复事务仍沿用原 ID。[远程锁回执](../../output/no-account-20261027/remote-locks/receipt.json)保存失败复现、隔离测试和源码摘要。

本地产品接线不等于远程上线。真实 Claude 账号、真实 Keychain/R2、普通 App 动态 IPC 和端到端传播仍未验收。锁只协调遵守协议的发布器，不能阻止有写权限的人工绕过。注册、部署、付款继续暂停；防攻击与费用方案仍待用户核对。

本阶段桌面后端完整套件 223 项通过，0 失败；其中相关连接测试最终 22 项通过，包含在 223 项内，不相加。完整套件后只合并两处条件语句，并针对最终代码复跑连接测试。Clippy、Public 编译和格式检查通过；保留 3 条原有迁移风格警告。前端未改动，沿用上一阶段有效构建记录。图谱为 227 文件、6100 节点、25090 条边，complete、pending 0。

## 历史阶段：保留远程发布目标（本地验证阶段）

更换 R2 来源或切换到本机目录时，软件保留旧目标配置及其写入凭据。Personal 的发布来源面板显示保留目标，并提供“选回此目标”；该操作不联网，不将密钥返回界面。目标回切后，发布继续使用原目标的历史、水位和待恢复事务。

软件兼容旧单目标配置，后续保存升级为有界目标配置表。同一目标轮换凭据只替换该目标条目。预览与正式渠道的当前和保留目标均须保持桶及访问密钥隔离。每渠道最多 16 个配置，总编码不超过 16 KiB；超限或损坏时拒绝保存并保留原资料。软件不自动删除旧目标。

旧目标账本没有匹配配置时，面板提示重新保存原配置。已有未完成发布时，配置回切不能绕过恢复门禁。配置覆盖核对不是远端连通或锁验证；远程多目标清理仍未实现，产品门禁保持关闭。

[目标配置回执](../../output/no-account-20261027/target-registry/receipt.json)记录回归复现、隔离测试、编译和输入摘要。真实 Claude 账号、真实 Keychain、真实 R2、普通 App 动态 IPC 和端到端传播不在本阶段已验证范围内。注册、部署和付款继续暂停。

本阶段发布器完整套件 174 项通过，包含 14 项配置测试，不重复计数。两版前端构建、Public 编译、Svelte（0 错误/0 警告）、格式检查通过。最终 Clippy 退出码 0，保留 3 条原有迁移代码风格警告。图谱为 223 文件、6009 节点、24577 条边，complete、pending 0。

## 历史阶段：旧发布资料完整恢复（本地验证阶段）

Personal 提供完整备份、确认恢复、取消恢复、导入完整归档、读取保留资料和加密导出入口。先保存并读回验证完整加密归档，再展示确认。准备与确认之间持有原 Publisher 和本机发布树锁；更改源文件、密钥、目录身份、备份或签名 current 会使确认失效。取消准备后，后台结果不得重新激活会话。

完整归档保留旧代次全部允许项，包括 state、checkpoint、cleanup-gc、未索引文件、密钥包及 trust。未知项、链接、缺失或损坏的登记素材使准备失败。归档在加密内容中保存路径、类型、长度和摘要。备份口令至少 12 个字符，文件不超过 64 MiB；原始文件总量另受编码预算限制，超限时停止，不截断。

确认后原子切换本机有效代次。新代次保留草稿所需素材，序号高于全部目标水位。旧历史、来源不明事务、旧目录和旧密钥继续保留。软件不重放旧事务，不自动上传；操作员核对新草稿后才能明确发布。完整归档导入只恢复到未激活的保留目录；同名代次或密钥冲突时停止。

```text
完整备份 → 读回验证 → 用户确认 → 再核对 → 原子切换
                                                  ↓
                                 保留旧资料，等待明确发布
```

旧 JSON 省略默认字段或采用不同排版时，软件按解析后的状态比较，并在归档中保留原始字节。该兼容问题已有失败复现和回归测试。核心测试、编译及图谱结果见[恢复验证回执](../../output/no-account-20261027/publisher-recovery/receipt.json)。本轮没有真实 Claude 账号、真实 R2、真实 Keychain、普通 App 动态 IPC 或端到端传播证据。远程多目标清理仍未完成；注册、部署、付款继续暂停。

阶段发布器套件 166 项通过。随后抽取共用归档内容校验并补齐缺失/损坏素材保护，最终归档与迁移测试 18 项、素材保护测试 1 项通过；这些计数有重叠，不相加。前端相关 24 项、两版前端构建、Public 编译与格式检查通过，Svelte 为 0 错误/0 警告。最终 Clippy 退出码 0，保留 3 条原有嵌套条件风格警告。图谱为 222 文件、5973 节点、24263 条边，complete、pending 0。

## 历史阶段：远程发布接线（本地验证阶段）

Personal 已接通渠道配置、密钥库存储和发布目标选择。操作员先读取配置，再分别设置预览与正式渠道。保存配置不连接远端；发布命令才自动上传、核对版本、验证签名并匿名回读素材。空配置明确使用本机目录；损坏配置直接报错，不能静默退回本机并声称远程成功。

配置与写入凭据一起保存到现有 Vault 密钥包。读取接口只返回账号 ID、桶名、公开域名和渠道。界面不回显密钥。两个渠道须使用不同存储桶和访问密钥。加密备份包含配置与凭据。已有未完成发布时，不得删除或更换其目标；同一目标允许轮换凭据并恢复原事务。

目标选择现在通过产品命令进入 `TargetLedger`。本机与 R2 的历史、版本水位和待处理事务分别保存。R2 必须明确选择目标后才能发布；切换目标不会把原事务提交到新目标。来源不明旧状态继续保留并拒绝自动归属。旧状态完整迁移入口、远程多目标清理仍未完成。

```text
Personal 配置 → Vault（两个渠道独立）
                     ↓
publish_ad → publish_configured → 选择目标账本
                     ↓
           LocalObjectStore / R2Store
                     ↓
       共用发布事务 → 签名核对与匿名回读
```

本阶段桌面套件首次为 186 通过、1 失败。失败仅来自新测试的提交尝试计数断言；修正后定向 1 项通过，累计 187 个不同测试名通过，没有重跑其余 186 项。新增配置 7 项已包含在桌面结果内。前端相关 27 项、两版前端构建、Public 编译、格式检查通过；Svelte 检查为 0 错误、0 警告。默认与 ipc-business Clippy 退出码均为 0，仍有旧清理/迁移代码警告，未宣称严格零警告。图谱为 218 文件、5879 节点、23801 条边，complete、pending 0。

[本阶段验证回执](../../output/no-account-20261027/remote-wiring/receipt.json)记录测试范围、失败历史及输入摘要。测试使用合成密钥、本机协议替身和隔离目录，不使用真实 Claude 账号。真实 Keychain、真实 R2、普通 App 动态 IPC、正式发行与端到端传播仍为 `NOT_RUN`。注册、部署和付款没有执行。防攻击方案见[安全方案](Cloudflare-R2-防攻击与费用控制方案-2026-10-27.md)；防护尚未部署。

## 历史阶段：密钥恢复验证与本地提交

密钥已写入而信任文件缺失时，恢复仍须保护现有凭据。仅明确的密钥不存在结果可视为空；读取失败须中止恢复。[验证回执](../../output/no-account-20261027/r2-target-identity/orphan-vault-summary.json)复用 9 项回归和 141 项发布器测试通过日志，本轮只补 Clippy（退出码 0，7 条未使用代码警告）和图谱同步。真实 Keychain、远程 R2 及产品接线仍未验证。此提交保存当前本地实现，不表示远程发布功能完成。

## 历史阶段：远程接线前的验证记录

产品目标是由 Personal 自动上传签名内容，由 Public 自动核对版本并验证签名。线上目标尚未验收，以下历史隔离测试不能作为线上验收通过证据。

客户端现支持构建期固定发行来源、公钥、密钥 ID 和渠道，并按信任身份隔离版本记录与刷新计划。正式 Public 构建入口检查发行材料并拒绝 `r2.dev`；这不证明正式域名已经部署 WAF。旧本地记录缺少可认证缓存时，保留原文件与旧版本下限，重新验证新内容后才采用。隔离广告模块 35 项测试与严格 Clippy 已通过；使用旧软件版本不会继承新版身份水位，跨旧版防回退尚未证明。完整 Tauri 打包和真实生产来源仍未验证。

客户端自动刷新测试已提交 `8c088fa`。[测试回执](../../output/no-account-20261027/ad-auto-refresh-r1/receipt.json)记录两项定向测试通过：到期后获取新的合成签名内容并重建运行时；重建后拒绝同序号的不同签名内容。测试使用合成密钥和内存数据源，未验证真实进程重启、guardian 调度、普通 App 或真实远程服务。

在 aab861cc 阶段，发布命令仅装配 LocalObjectStore。以下为该阶段的历史限制；本轮接线状态以上方为准。候选目标隔离会阻止来源不明的旧版发布状态继续发布，但目前缺少用户可用的恢复入口；旧状态须保留，不能把此兼容问题标为完成。修复后须覆盖旧待处理事务、旧历史、故障重启及跨目标隔离。

R2 适配器现能生成不含凭据的目标标识。该标识区分账号、存储桶、公开域名和渠道，密钥轮换不改变标识。本地合成配置与协议替身测试覆盖该行为；该历史证据本身不证明产品接线；本轮目标选择及账本隔离见上方。预览和正式渠道若使用不同存储桶，须分别保存并恢复各自的发布状态。

工作树补充两项发布恢复保护：其他渠道存在未完成发布时，先保留该事务；备份与本地在同一渠道使用相同版本号但内容摘要不同时，拒绝恢复并保留原状态和密钥。两项均有失败复现及回归；[发布器隔离测试](../../output/no-account-20261027/r2-target-identity/publisher-summary.json)为 75 项通过。它们尚不能替代多目标状态隔离。

工作树新增多目标状态原型：目标各自保存历史、水位和未完成事务，草稿和素材共享。多目标备份恢复合并各目标的历史和水位，并保留本机素材；事务、历史、版本或凭据冲突时拒绝恢复。恢复后每个目标须重新核对。[隔离测试摘要](../../output/no-account-20261027/r2-target-identity/backup-merge-summary.json)记录 7 项多目标备份回归通过；新增最后一项测试前，发布器套件为 133 项通过。该历史阶段目标选择尚无产品入口；本轮已接线，多目标清理仍拒绝。跨目标素材清理、产品接线和真实远端验证仍未完成。

清理引用计算覆盖其他目标的历史、待发布和恢复证据，并阻止在其他目标事务未完成时提交。多目标接口要求在持锁期间核对所有目标的当前版本；测试替身已验证缺失目标、版本回退、签名篡改、预览过期和垃圾回收前引用变化。本地多目标适配器已通过隔离目录锁和身份替换测试。产品命令尚未接入该适配器，远程目标尚无多目标锁方案，产品中的多目标清理保持关闭。

多目标备份恢复会先查询当前密钥库。只有密钥库明确报告项目不存在，恢复才把它视为空。信任文件缺失时也会保护已存在的凭据；读取失败会中止恢复。9 项合成密钥库回归通过，真实 Keychain 错误行为仍待验证。

Cloudflare 注册和付款按用户要求暂停。尚未创建或验证真实 R2 存储桶、发布凭据、正式公开地址和客户端生产验证材料。缓存、限速、费用提醒及应急关闭均为待配置方案，不能视为已启用。真实远程上传、公开回读、签名验证、传播时间及线上攻击防护均未验收。方案见 [远程广告发布设计](Cloudflare-R2-远程广告发布设计-2026-10-27.md) 和 [防攻击与费用控制方案](Cloudflare-R2-防攻击与费用控制方案-2026-10-27.md)。

广告内容自动更新与软件安装包自动更新是不同链路。上方目标针对广告内容；软件安装包仍遵守下文 SourceLocal 与 DeveloperId 各自的更新条件。

## 本轮增量：恢复门禁和隔离证据

代码修复提交 `0a68899`；探针修复提交 `7fec6db7f9b54919eea08301f3d06d16ddfc8a8a`，tree `8bfed7dd5e085504cf5063bb5fd3142ac21a3e4d`。普通双包已从7fec精确归档构建；[构建回执](../../output/no-account-20261027/bundles-next-commit-r2/final-summary.json) SHA256 `128278784d1d5525b360f8e2b8a3cb16c187c77de9a4f0c2bd6881b4bdfb5c78`，归档SHA256 `ddc21ebb676f4cce6b9adce7ef04d4eda0ee3c736bc372ec385c0d8ec4023ff4`。每版22项静态检查和2项复制/签名操作通过；夹具运行体标记排除PASS，二进制名称标记仍为REVIEW。普通App、动态ACL及安装NOT_RUN。93afc71普通双包为历史，不归属于7fec。

地区恢复门禁的[父目录链接RED](../../output/no-account-20261027/repair-gate-parent-r1/red.log)暴露两个拒绝缺口，[GREEN](../../output/no-account-20261027/repair-gate-parent-r1/green.log)为3项通过；账号侧复用该检查。账号journal与安装事务的[账号RED](../../output/no-account-20261027/update-journal-r1/account-red.log)、[安装RED](../../output/no-account-20261027/update-journal-r1/install-red.log)分别保留，后续[账号测试](../../output/no-account-20261027/update-journal-r1/accounts-final.log)为41通过、[安装测试](../../output/no-account-20261027/update-journal-r1/install-suite.log)为14通过。update idle调用方[RED](../../output/no-account-20261027/caller-gates-r1/red.log)含2失败，[GREEN](../../output/no-account-20261027/caller-gates-r1/green.log)为4通过。该证据覆盖受控合成状态，不证明并发路径替换全面受阻。

安装测试目录碰撞[RED](../../output/no-account-20261027/update-journal-r1/collision-red.log)和[GREEN](../../output/no-account-20261027/update-journal-r1/collision-green.log)保留；修复使夹具避免占用或清理已存在目录。更新提示及夹具碰撞[RED](../../output/no-account-20261027/update-feedback-r1/red.log)含3失败，[GREEN](../../output/no-account-20261027/update-feedback-r1/green.log)记录5个harness各24项通过，不能当作120个不同测试名。探针[RED](../../output/no-account-20261027/probe-failclosed-r1/red.log)含父、子进程两项断言失败，[GREEN](../../output/no-account-20261027/probe-failclosed-r1/final.log)为8项通过；单凭denied不能证明securityd隔离。真实Keychain、安全服务和系统托管WebKit回收仍未证明。

地区[全禁网尝试](../../output/no-account-20261027/repair-gate-parent-r1/regions.log)为60通过、2代理测试因Operation not permitted失败；[筛选重跑](../../output/no-account-20261027/repair-gate-parent-r1/regions-no-network.log)为60通过、363 filtered，不替代被排除的代理测试。[首次rustup误触](../../output/no-account-20261027/update-journal-r1/clippy.log)与[再次rustup误触](../../output/no-account-20261027/caller-gates-r1/clippy.log)记录连接被拦截、下载未完成；后续Clippy使用固定工具链，不能声称未尝试网络。

[最终Rust回执](../../output/no-account-20261027/integrated-fixture-final-verify/summary.json)的tests、list、fmt及三项Clippy均exit 0：774条顶层通过、658个不同通过名称、1 ignored、15个harness、3条nested。15个harness原始汇总为774 passed、0 failed、1 ignored、2 filtered；3条nested单列。两项指定原生测试由 `--skip` 排除，list中均不出现。ignored项目 `activity_write_denial_returns_error_after_preferences_commit_both_editions` 需要单层Seatbelt。该沙箱禁止外网，但允许localhost连接和bind；仅验证列出的负控，不能称Keychain专项通过。执行时HEAD为 `9c132363609b94b0fe0857de4224aa9e1c3081bd` 加未提交变更；275项输入前后不变，不是从后续提交归档直接运行。提交后独立核对其中257项与7fec Git blob一致，0项不匹配；另18项不在提交中，包括4项Tauri schema及14项自动生成权限文件。回执另记录59项生成文件总数，不代表59项均未提交。[当前图谱](../../output/no-account-20261027/graph-recovery-gates-r1/status.json)为209文件、5551节点、21565边，complete、pending 0；状态文件SHA256 `76485ecb88b7e85c3d5b0d869fbc55931226c396bae8c1da60340ae810e9c6c0`。普通App、动态ACL、签名安装、真实账号、Keychain、真实代理及远程CAS仍未验。

## 历史增量：发布闭环与图片边界

图片测试提交 `3c762e7`；发布IPC与共享业务提交 `93afc715fbcb9c9cbc506f78e736447ca1021672`，tree `745b37ef9fe7682cbf99cb89643dbae511fb0a5c`。历史普通Personal/Public包已从该提交的精确归档构建；[构建回执](../../output/no-account-20261027/bundles-next-prep-r1/final-summary.json) SHA256为 `9aa9a5bbbff29fbc5fb40824fdd9353dba3c564858c031fbd6b028d4995409e3`，归档SHA256为 `735defd7c6f671f575afccba0ba8ba36dfaaee36c2b9088720fae0fc73bd2702`。每版22项静态检查和2项复制/签名操作通过，夹具运行体标记排除通过；二进制名称标记仍为REVIEW。普通App启动、动态ACL及安装均NOT_RUN。592e包保留为历史，不能归属于新提交。

[发布闭环回执](../../output/no-account-20261027/publish-ipc-r1/run-20261027-082701-publish-persistence-5125477d/result.json)记录四个独立隔离fixture进程，均exit 0。Personal覆盖本地发布、历史恢复、过期整理预览拒绝、有效整理、素材回收，以及新进程重新注入合成密钥后发布。Public的八个命令返回permission，观察到的磁盘状态不变。生产发布路径和fixture共享本地发布业务；fixture使用MemoryVault。[r1](../../output/no-account-20261027/publish-ipc-r1/run-20261027-082213-publish-persistence-cc562bea/result.json)因夹具请求清理仍被引用的历史而被拒绝；[r2](../../output/no-account-20261027/publish-ipc-r1/run-20261027-082625-publish-persistence-4f780d2b/result.json)的Personal进程exit 0，校验器误判省略的空cleanup字段，均作为历史保留。校验器核对payload哈希和签名长度，不提供独立Ed25519验签；合成活动时间已过期，不能证明当前广告展示。

[图片集成测试](../../output/no-account-20261027/image-boundaries/integrated-test.log)记录3项通过：2000×1000 JPEG边界成功，2002×1000超限、截断及伪装输入拒绝。成功导入移除了夹具的EXIF ImageDescription标记；这不证明全部EXIF或ICC均被删除。失败路径检查状态和合成目录不变。[目录碰撞RED](../../output/no-account-20261027/image-boundaries/directory-collision-red.log)复现测试目录碰撞问题；修复后只清理由测试独占创建的目录。[rustup失败日志](../../output/no-account-20261027/image-boundaries/integrated-clippy.log)记录误用rustup shim时连接被拒绝、下载未完成；[固定工具链Clippy日志](../../output/no-account-20261027/image-boundaries/integrated-clippy-r2.log)为固定本机工具链后续复验。不能称此前没有网络尝试。

[历史完整Rust门禁回执](../../output/no-account-20261027/integrated-final-verify/summary.json)记录tests、list、fmt、默认Clippy及两版ipc-business Clippy六项exit 0：748条顶层通过记录、644个不同通过名称、1 ignored、15个harness、3条nested记录。原始汇总有751条通过记录，其中3条为nested；列表有749条记录，含1 ignored。两项特定原生系统测试通过 `--skip` 排除；各harness另有其他filtered记录。该沙箱允许localhost，不能称为完全禁网。执行时HEAD为 `d7e506add25d00fba82474d900a54d2249a0dffe`，附加272个输入文件的前后内容绑定，包含当时未跟踪的源码与图片；不是从后续提交归档直接执行。提交后核对发现，其中251项与93afc71的Git blob内容匹配，0项不匹配；另21项不在提交中，包括14项生成权限TOML及7项Python字节码。新增JPEG、Rust、JavaScript和Python源码均匹配。[历史图谱状态](../../output/no-account-20261027/graph-integrated-r1/status.json)为209文件、5527节点、21414边，complete且pending为0；状态文件SHA256为 `4d04558f2bc70f5e39dc340880164a27ade658f18a97664cb0bdd1ee0b5d3520`。图谱统计不表示测试覆盖率。

复现入口：先用 [build_native.py](../../apps/desktop/tests/native-ipc-business/build_native.py) 分别构建Personal和Public，使用相同的 `--output` 与新 `--attempt`，并指定已有 `--toolchain-bin`、`--cargo-home` 和 `--target-dir`；构建串行执行。再调用 [publish_persistence.py](../../apps/desktop/tests/native-ipc-business/publish_persistence.py)，以相同 `--output` 和 `--build-attempt` 绑定两版构建回执及二进制。运行前应核对沙箱策略和输入；该入口只运行测试fixture。真实Claude账号、真实Keychain、普通App启动与动态ACL、远端ObjectStore和系统托管WebKit回收均未验证。沙箱配置限制Keychain连接，但没有单独运行Keychain连接主动负控，不能据此宣称该负控通过。

普通App隔离前置探针提交6589e97：[schema真实RED](../../output/no-account-20261027/app-isolation-probe/schema-red.log)暴露嵌套类型异常，[7项纯测试GREEN](../../output/no-account-20261027/app-isolation-probe/schema-green.log)通过。[直接进程回执](../../output/no-account-20261027/app-isolation-probe/direct-process-m0kg8c7w/receipt.json)仅证明父进程及直接子进程的所选边界和直接子进程回收；securityd、WebKit隔离及系统管理子进程清理NOT_PROVEN，runner非零，普通App NOT_RUN。回执内探针进程exit0不等于runner整体通过；非本地实际连接和真实Keychain API未执行。

SOCKS受控测试提交 `a728607170cf91365c103f7332230c6bc6e082cd`：[4项定向测试](../../output/no-account-20261027/socks-controlled/evidence-green.log)及[Clippy](../../output/no-account-20261027/socks-controlled/evidence-clippy.log)通过，覆盖合成SOCKS5/5h、本地/远程DNS、TLS、认证及旁路拒绝。[可写PATH伪工具RED](../../output/no-account-20261027/socks-controlled/path-red.log)与[证据路径覆盖/链接RED](../../output/no-account-20261027/socks-controlled/evidence-red.log)均保留，最终测试不向任意指定证据路径写入。[命令回执](../../output/no-account-20261027/socks-controlled/verification-receipt.txt)绑定前后输入哈希；旧wire.jsonl仅历史。早期rustup shim查询更新因DNS失败而中止，后续固定本机工具链；该事件保留，不能称全程没有联网尝试。真实网络、真实代理和Keychain未验。

历史[图谱](../../output/no-account-20261027/socks-controlled/codegraph-status.json)为204文件/5471节点/21248边，complete、pending0。6589及a728阶段仅改测试工具；该历史普通包精确绑定592e，不改标为后续HEAD，741/637结果亦保留为历史。

历史592e普通双包[构建回执](../../output/no-account-20261027/bundles-commit-592e1f5-r1/final-summary.json)绑定提交 `592e1f5de74e8b0cf2bde2a25b0e6c11b6da1269`、tree `432e8a55ded9f1e785971fd7e700d32e4dd65744`，archive SHA256 `fa3ded50e06807a3bd800d80b2e5f57dc4a9b901a1fc47c5d7bcf4d9127e4e3f`。每版22项静态检查及两项复制/无时间戳ad-hoc签名操作通过；运行体标记排除PASS，历史字符串保持REVIEW_MARKERS_PRESENT。Personal manifest SHA256 `c401619c3ac3446e17f00c986c54a5ea5e3519cba124e452262ad6e69edfeacc`；Public `ecf3dcb2697d3c4329768124592729551f6784338c4344e2aad20d403031a815`。本轮独立只读审查通过：重新比对git archive SHA、两版各24条命令日志（22检查及2复制/签名操作）、每版7个App文件、Personal5/Public3个dist文件及签名；核对[Personal产物回执](../../output/no-account-20261027/bundles-commit-592e1f5-r1/artifact-personal.json)及[Public产物回执](../../output/no-account-20261027/bundles-commit-592e1f5-r1/artifact-public.json)。审查者未写独立回执，没有重新运行产品；forbidden_probe/finish_smoke字符串仍REVIEW。普通App启动、安装、动态ACL NOT_RUN。c854及更早普通包保留为历史。

原严格沙箱[O_SEARCH修复IPC复验](../../output/no-account-20261027/search-fix-ipc-r1/final-summary.json)绑定提交 `592e1f5de74e8b0cf2bde2a25b0e6c11b6da1269`，15/15 IPC场景、26组负控、172项解析器测试通过；先前失败的Personal business在同一用户目录边界通过。[独立只读复核](../../output/no-account-20261027/search-fix-ipc-r1/independent-review/review.json)PASS，核验23个PID、41项结果及70个日志哈希。该证据仍为feature受控fixture，普通App、真实Keychain和WebKit子进程清理未验证。592e普通包构建及独立复核状态见上方，不能用IPC复验结果替代包验收。

历史[第三轮Rust最终回执](../../output/no-account-20261027/storage-final-verify/third/final-receipt.json)记录六项门禁exit0，741顶层通过/637不同通过名称、1 ignored、15 harness、3嵌套；两项系统探测按约定过滤。执行HEAD为 `b682e0b398bcfd5e0cbd40ab999655a8cbc20996` 加当时未提交O_SEARCH，源码快照SHA256 `72b173d4fbe3187f7d24eec49ecf7d83702275d425fe89d61aaeeba19a9a8bef`。源码随后提交 `592e1f5de74e8b0cf2bde2a25b0e6c11b6da1269`，不能称为592e归档执行。该轮与下方740/636历史轮次分列；严格IPC现已复验，见上方原沙箱段；592e新包构建及独立只读审查通过，普通App和其他未验门槛不由此通过。

O_SEARCH兼容性修复已有[真实断言RED](../../output/no-account-20261027/storage-directory-race/search-red.log)：0111祖先目录导致PermissionDenied；macOS改用O_SEARCH后[存储23/23 GREEN](../../output/no-account-20261027/storage-directory-race/search-green.log)，覆盖本机当前文件系统的仅搜索权限祖先目录，独立审查已核对该范围。Linux仍使用O_RDONLY，未安装target，编译和对应行为均未验证。原严格IPC复验结果见上方，第三轮全量结果见741/637段；这不构成最终完成结论。

b682e0b真实HOME下测试输出根的[IPC业务尝试](../../output/no-account-20261027/storage-fix-ipc-r1/run-20261027-075444-personal-business-05690f3b/result.json)失败，原因是O_RDONLY打开祖先目录所需权限；不能标产品全流程PASS。首个错误cache路径导致的编译前失败保留。[/private/tmp合成根基线](/private/tmp/claude-switcher-ipc-b682e0b-r1/final-summary.json)绑定b682e0b，15场景/26负控/172解析器通过，SHA256 `74bd539ddfa17d474bef3f288c615e4c2df265e9c48520c31aedec690b9b3144`。该临时目录可能被系统清理，不属于仓库持久证据，本次未复制。该失败随后由592e原严格沙箱复验修复，见上方；合成根基线仍不替代该复验。

历史第二轮[最终Rust重试回执](../../output/no-account-20261027/storage-final-verify/retry/final-receipt.json)记录六项exit0，740顶层通过/636不同通过名称、1 ignored、15 harness、3嵌套子进程；两项系统探测按约定过滤。实际执行HEAD `82f0738fa00b48ae4c3f9ca3076919518b0a8e88` 加未提交工作树，snapshot SHA256 `445a5e60af9b7d45583928fa1ec252fbaa479d74093388830048f4804ce17990`。源码相关提交为9a9cfa6、82f0738、a8706b0、b682e0b；后两项内容执行时尚未提交，不能描述为从后续提交归档执行。首次fmt/Clippy失败仍保留；双版IPC及592e普通包静态审查已完成，普通App和其他未验门槛不由此通过。

本轮首次全量tests/list均exit0，但[fmt日志](../../output/no-account-20261027/storage-final-verify/final/fmt.log)记录exit1：accounts新增断言格式不符；[默认Clippy日志](../../output/no-account-20261027/storage-final-verify/final/clippy.log)记录exit101：storage测试hook的两处thread_local初始化可用const，另有type_complexity。修正后完整重试通过，见本轮最终Rust段；首次失败历史保留，不以driver进程exit0替代各检查结果，也不提前标最终PASS。

本轮账号旧恢复入口对悬空journal的[真实断言RED](../../output/no-account-20261027/recover-dangling/red-retry.log)及[账号40项GREEN](../../output/no-account-20261027/recover-dangling/final.log)单列；首轮red.log仅为沙箱写编译产物权限失败，不是行为RED。[用户修复悬空日志RED](../../output/no-account-20261027/user-repair-dangling/red.log)与[地区恢复RED](../../output/no-account-20261027/user-repair-dangling/region-red.log)各有真实失败，后续[用户修复36项](../../output/no-account-20261027/user-repair-dangling/clean-green-user.log)及[地区27项](../../output/no-account-20261027/user-repair-dangling/clean-green-region.log)通过。地区级测试采用runtime_state=None，不能推广至所有代理状态。证据仅覆盖隔离合成状态，真实账号和服务调度未验。

存储路径竞态[原始RED](../../output/no-account-20261027/storage-directory-race/red.log)实证父目录替换可使读取跟随诱饵、写入诱饵目录；[移动父目录RED](../../output/no-account-20261027/storage-directory-race/moved-red.log)及[系统错误分类RED](../../output/no-account-20261027/storage-directory-race/error-red.log)分别保留。[最终定向22项GREEN](../../output/no-account-20261027/storage-directory-race/error-green.log)覆盖目录锚定、祖先链接拒绝、读写前后身份检查、rename前移位拒绝及临时文件清理、提交后移位/目录同步失败的可能已提交错误，并保留底层系统错误。PostCommitError外层raw_os_error为None，错误链保留内层原因；fsync失败为测试注入。历史O_RDONLY阶段未覆盖祖先目录仅搜索权限；本轮macOS O_SEARCH定向验证见上方，Linux target仍未安装，Linux编译NOT_RUN。该防护有限，不证明所有时刻的目录并发替换均已阻止，也不证明断电或fsync持久性。

历史[O_SEARCH最终图谱](../../output/no-account-20261027/storage-final-verify/codegraph/search-status.json)为199文件/5380节点/21006边，complete、pending0；[先前格式化图谱](../../output/no-account-20261027/storage-final-verify/codegraph/final-status.json)199/5377/20995保留为历史；[格式调整前状态](../../output/no-account-20261027/storage-final-verify/codegraph/status.json)199/5376/20994及此前199/5337/20729均保留为历史。本增量最终完整测试及源码状态见第三轮741/637段，双版IPC及592e包静态审查结果见上方，不套用下方721/617或c854包结论；本历史阶段已验证普通包绑定c854；当前包见上方592e段。普通App、真实账号、Keychain、远程发布和安装仍未验。

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

本轮仅测试提交 `dd1d7e37affcca03976acddfe9f4a822d7a19a31`，tree `cb96d94e2c812d4f98371f6749659fe0b2c3eee2`，补足真实LocalObjectStore的受控退出恢复。[定向回执](../../output/no-account-20261027/local-store-cas-exit/final-receipt.json)包含2个顶层测试，其中1个为guard子入口；父测试驱动提交前/后两个 `process::exit(73)`切点。子进程退出后，一直存活的父测试进程重新打开并恢复；恢复CAS分别为1次/0次，重获OS锁，核对同一release/revision及签名wire、签名manifest、不可变版本和全部素材，最终恰好1条历史、pending清除，再次重开保持状态。它没有运行Drop；不等同SIGKILL、断电、fsync故障、HTTP CAS或真实Keychain验收。

[全量r1最终回执](../../output/no-account-20261027/local-store-cas-exit/rust/r1/final-receipt.json)记录tests/list/fmt/默认Clippy及两版feature Clippy共6命令通过。[严格对账](../../output/no-account-20261027/local-store-cas-exit/rust/r1/strict-result.json)为709 passed/605个passed名称、1 ignored、3 nested，按harness清单一致；ignored和nested不重复计入passed。原粗统计失败保留，真实sysctl采样及服务认证探测两项继续跳过；专用r7证据单列，不冒充默认全量执行。历史CAS阶段[图谱](../../output/no-account-20261027/local-store-cas-exit/codegraph-status.json)为189文件/5141节点/20172边，complete、pending0。dd1不改变生产源码与普通包绑定：该历史轮次双包归档自8961905；E01 REVIEW及普通App/动态ACL/安装/账号/实源/长时未验边界保留。

本轮资产与代理源码提交：代理 `1836db7f59867bb437c3938610e8d6b603c820a8`；合成PNG资产恢复 `89619059dd473ad04503fa3326476d0526980016`，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`。[代理回执](../../output/no-account-20261027/controlled-proxy-tls/final-receipt.json)记录生产client builder等价提取及受控CONNECT/TLS验证。合成CA只在cfg(test)替换信任，不改变生产默认信任；本机受控传输不证明真实代理或账号可用。r1及isolation-red系统编码环境失败保留：macOS在env_clear后生成 `__CF_USER_TEXT_ENCODING`，fixture随后仅放行该编码提示；此失败不称为产品传输缺陷。

[资产native-r2](../../output/no-account-20261027/backup-assets-ipc/native-r2/summary.json)与[独立复核](../../output/no-account-20261027/backup-assets-ipc/review-native/review.json)证明Personal通过共享恢复body及MemoryVault恢复真实合成1×1 RGBA PNG：1成功/4拒绝、同进程5次重开，最终源/恢复/旧资产的字节、CRC、受限解压、像素、哈希、类型、大小、尺寸及引用独立核对，附加元数据已移除。121项解析器回归包含16项新增资产检查。四个失败场景的不变性来自fixture；只有最终磁盘独立检查。120项历史输入内容图绑定该轮，不能称为完整当前git源码；独立复核时 `network.rs`与 `tests/fixtures/explicit_proxy_tls.py`已有后续漂移。r1虽Cargo exit0，但因构建中输入漂移拒绝二进制归属，失败证据保留。历史parser/red是ImportError，不是行为RED。

[本轮Rust r1严格对账](../../output/no-account-20261027/asset-proxy-verification/rust/r1/strict-result.json)确认707 passed/603个passed名称、1 ignored、3 nested，按harness清单一致；四命令exit0，[两版feature Clippy](../../output/no-account-20261027/asset-proxy-verification/rust/r1/ipc-business-clippy/summary.json)通过。与先前705轮次分开，ignored及nested不重复计入passed。历史资产代理阶段[图谱](../../output/no-account-20261027/asset-proxy-verification/codegraph-status.json)为189文件/5130节点/20104边，complete、pending0。历史8961普通双包已从 `89619059dd473ad04503fa3326476d0526980016` 精确归档构建，tree `628293ce32bc55e49eb79ac82c93042fd5de8a57`；[交付回执](../../output/no-account-20261027/bundles-asset-proxy/delivery-summary.json)绑定archive SHA256 `7bc9e7dcc4551d25976d53df1ad19d33a5f696c9b558aa87ea52152d8402fa94`。每版22项静态检查及2项复制/adhoc签名操作完成，最终重验通过；fixture运行体标记排除，E01为REVIEW_MARKERS_PRESENT。[最终独立包复核](../../output/no-account-20261027/bundles-asset-proxy/review/final-review-v2.json)通过限定静态检查。Public当前临时dist可独立重扫；Personal原dist被Public构建覆盖，仅保留构建时dist回执/source/config/日志，不能独立重扫历史dist。两版已签名App均已独立重扫。E01仍REVIEW，普通App启动、动态ACL及安装仍NOT_RUN。marker初始控制失败仅保留cause/correction/product_source_changed三字段说明，无原始traceback；修正后的28项指定token控制已核对，不能把初始失败机制称为独立观察。审查器首次review.json的模式/feature位置误判保留，由final-review-v2纠正；不把旧误判当成产品缺陷。6136及更早普通包保留为历史。MemoryVault、同进程备份重开及单个合成PNG不证明真实Keychain、备份跨App重启、全部编解码器/尺寸或完整资产验收；失败可能留下不可达代次。普通App、动态ACL、安装、真实账号、实源、系统/长时及独立WebKit清理仍未验，E01保持REVIEW。

历史7540草稿IPC产品提交为 `7540c9ec3ff390a96b9dbadd8d340577601b0750`，tree `c98501f300459ba076005becdb78fbe8862bb6ba`。[native-r2回执](../../output/no-account-20261027/draft-ipc/native-r2/summary.json)与[独立复核](../../output/no-account-20261027/draft-ipc/review-native/review.json)记录两版共14个受控fixture进程、115项输入内容图、105项解析器回归及16组实际沙箱负控。Personal直接调用生产 `ad_draft_read`、`ad_draft_save`、`pause_ad`，9成功/9分类拒绝；拒绝要求准确业务错误或对应命令解码错误。Public三个命令均权限拒绝；运行时拒绝不能证明handler不存在，后者单列静态注册/capability证据。

18个有序场景的36份逐步摘要快照来自fixture；Python只对最终磁盘作独立检查，并核对快照链及事件/响应一致性。不把逐步fixture快照称为逐步独立磁盘观察。图片只含合成元数据；没有图片导入/显示、商家发布、ObjectStore或真实Keychain证明。四个跨进程阶段仍属于偏好，不能外推草稿重启/崩溃持久化。native-r1因保留域.example输入被merchant_host拒绝的失败保留，属于fixture输入问题；host与error-class各有真实断言RED/GREEN，历史 `parser/red.log`为模块缺失ImportError，不能作为行为RED。

[本轮Rust r1严格对账](../../output/no-account-20261027/draft-ipc/rust/r1/strict-result.json)为705 passed、1 ignored、3 nested；601个passed名称，按harness清单一致，nested不重复计数。tests/list/fmt/默认Clippy四命令exit0，[两版feature Clippy](../../output/no-account-20261027/draft-ipc/rust/r1/ipc-business-clippy/summary.json)通过；ignored不计入passed，既有专用r7证据单列保留。历史草稿阶段[图谱](../../output/no-account-20261027/draft-ipc/codegraph-status.json)为186文件/5055节点/19898边，complete、pending0。该历史轮次普通包精确绑定6136，7540普通包未构建；E01保持REVIEW，普通App、动态ACL、安装、真实账号、真实Keychain、实源与长时未验，独立WebKit子进程清理仍未证明。下方6136/native-r3及其输入快照差异完整保留。

历史6136合成备份IPC源码提交为 `6136fe85fe573e9ebe42316f8e482070e9c34127`，tree `292a93f1013c725bc0579f457eb3cf60039008ee`。此前 `b4d06baa2494d55247d5a41e13a7702cc27c7b23`补足偏好故障与专用r7测试；`3a8161ea0a7ab0acffd21e0ef04cc6de73c3385d`使严格解析器单列ignored。[native-r3回执](../../output/no-account-20261027/encrypted-backup-ipc/native-r3/summary.json)与[独立复核](../../output/no-account-20261027/encrypted-backup-ipc/review-native/review.json)绑定当时的112项输入内容图，不证明完整6136源码构建。其中 `e11_log_privacy.rs`仍为后续Clippy修正前内容，三个components二进制属于生成输入；112项不能统称git源码。该e11修正由后续Rust r2与专用r7验证。89项runner回归包含18项备份解析测试。Personal使用共享恢复body和MemoryVault完成1成功/4拒绝：错误口令、篡改、截断及Vault故障。每次在同一fixture进程重开Publisher检查状态；Public两个备份命令均在权限层拒绝且无写入，不能由此证明运行时handler不存在。

备份失败保护活动状态、旧代次及旧Vault条目；不要求整个目录树不变，可能保留非活动孤立代次。本轮合成备份不含图片资产；四独立进程证据仍只证明选定偏好持久化。native-r1目录初始化失败与native-r2旧Public oracle失败均保留；目录fixture有Rust RED/GREEN，权限oracle有2项RED及18项GREEN。MemoryVault不证明生产原始Keychain命令、备份跨进程恢复、普通App或完整资产覆盖；独立WebKit子进程清理仍未证明。

[Rust r2严格对账](../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/strict-result.json)确认705顶层passed/601个passed名称、1 ignored，合计706记录/602名称；3条nested不重复计数，按harness清单一致。ignored活动记录写拒绝测试需单层Seatbelt，已由[专用r7](../../output/no-account-20261027/ordinary-next/preferences-faults/r7/receipt.json)独立通过。该直接Rust测试覆盖两版：偏好目标阻塞时保留活动记录；活动记录写拒绝时返回错误，已提交偏好可由新状态读回。它描述部分提交行为，不证明双文件事务，也不属于原生IPC。tests/list/fmt/默认Clippy四命令exit0，[两版ipc-business feature Clippy](../../output/no-account-20261027/encrypted-backup-ipc/rust/r2/ipc-business-clippy/summary.json)通过；原粗计数失败回执及此前失败轮次保留，不把ignored计入passed。历史备份阶段[图谱](../../output/no-account-20261027/encrypted-backup-ipc/codegraph-final.json)为183文件/4997节点/19474边，complete、pending0。历史6136普通双包已从6136精确git archive构建；[交付回执](../../output/no-account-20261027/bundles-backup/delivery-summary.json)绑定完整提交 `6136fe85fe573e9ebe42316f8e482070e9c34127`、tree `292a93f1013c725bc0579f457eb3cf60039008ee`及archive SHA256 `ddcf3bc6490e466c163cf290727639b99ee3171abbe93da1d47a717e8d2e4fbe`。每版22项静态检查及2项复制/签名变更完成，各12条目重验通过；备份fixture运行体标记未检出。E01保留REVIEW；[独立包复核](../../output/no-account-20261027/bundles-backup/review/review.json)已通过限定静态验证。Public当前dist已独立重扫；Personal原始dist已被覆盖，只能引用构建时dist证据。普通App启动、动态ACL与安装仍NOT_RUN。旧d0a及更早包保留为历史。普通App、动态ACL、安装、真实账号、真实Keychain、远程发布及系统/长时门槛保持未验。

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

历史阶段增量：三文件仅测试提交 `30e861aca3c794d521f921d24e1dd112827bddb9` 的统一隔离组700次顶层执行/596名通过，新增8名无删除；list一致，fmt、Clippy通过，同两项原生排除。它与692/588及CNC684/580分轮记录，不包含子进程重复计数。两版已通过受控独立入口调用真实活动记录/导出命令的原生IPC；它直接注入合成DesktopState，不运行普通启动。显式代理407/502合成端点两项新场景及三个普通切换SIGKILL→全新进程恢复定向回归完成。生命周期Fake、注入TLS客户端与合成备份Vault的证据限制继续保留。稳定runner r9已完成限定验证；最终Rust全量r2已完成并经独立分组复核；历史00db普通双包已完成限定静态验证及独立静态复核。真实账号、正常App、实源、系统、安装与长时仍未验。详情及原始回执哈希见[本轮独立证据索引](../acceptance/evidence/no-account-20261027/ordinary-next-index.md)。原37项验收行保留，35项完整性未证明，33–66小时粗估保留。

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

当前已完成CNC适用Rust隔离回归及限定原生载荷边界。普通CNC双包已独立构建并完成静态验证，准确路径、哈希及REVIEW_MARKERS_PRESENT的限定解释见[当前CNC候选绑定](../handoff/无需账号功能进度-2026-10-27.md#0-当前cnc阶段)。后续已增加受控业务IPC和受控传输证据，范围见文首增量记录。普通App动态IPC、实际安装、真实账号与长期运行仍按各自条件推进。

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

