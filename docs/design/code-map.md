# Claude 切换器当前代码图谱

两版最新候选包基于 `afce48da` 构建，包含广告来源和刷新提示修复。构建、签名校验及隔离窗口启动通过；当前包的窗口业务操作尚未验证。[候选验收](../acceptance/当前候选验收-2026-10-27.md)。

## 2026-10-28：发布凭据和恢复后的配置

`create_ad_signing_key → Publisher::generate_signing_key → Vault::create → save_trust`。创建密钥只新增密钥库条目；旧条目存在时中止。已有配置的常规修改仍使用 `credentials_save`。

`publisher_remote_read → Publisher::remote_settings → PublisherModel.readRemote`。已保存的密钥缺失时返回错误；界面丢弃旧配置。备份或归档恢复后，`refreshAfterRecovery` 也丢弃旧配置，须重新读取。[回归与边界](../acceptance/evidence/no-account-20261027/publisher-credential-safety-20261028.md)。

本次 CodeGraph 状态 complete、pending 0：306 文件、7,568 节点、30,231 条关系。旧候选包不含本次修复。

## 2026-10-28：整理历史渠道提示

`preview_publisher_cleanup → history_label → PublisherCleanupPreview → 整理确认界面`。保留和移除记录共用渠道名称；界面不推断记录的存储地点。[回归记录](../acceptance/evidence/no-account-20261027/history-label-20261028.md)。`afce48da` 候选包早于该修复。

CodeGraph 状态 complete，待处理变更为 0：306 文件、7,563 节点、30,214 条关系。

## 2026-10-28：远程发布配置原生接线

`PublisherModel.readRemote / saveRemote / selectRemote → DesktopBridge → TauriWebViewTransport → publisher_remote_read / save / select → Publisher 目标配置`。原生隔离测试使用合成内存密钥库，覆盖两条渠道的配置保存、拒绝无效地址、目标切换和重新读取。[测试与边界](../acceptance/evidence/no-account-20261027/swiftui-remote-settings-20261028.md)。

CodeGraph 状态 complete，待处理变更为 0：306 文件、7,562 节点、30,211 条关系。静态图不代表真实云端或窗口操作已验证。

## 2026-10-28：广告手动刷新命令链

`refresh_ad_content → DesktopState::refresh_ads → AdRuntime::refresh_if_due → 签名校验与缓存`。桌面命令测试使用 Tauri MockRuntime、合成资料和测试专用时钟。生产代码继续读取系统时钟。两版构建验证关闭与缺少来源；签名成功、篡改拒绝及恢复在 Personal 测试程序的两版状态中验证。[证据和边界](../acceptance/evidence/no-account-20261027/ad-refresh-dispatch-20261028.md)。

CodeGraph 状态 complete，待处理变更为 0：306 文件、7,550 节点、30,111 条关系。静态图不代表运行验证。

## 2026-10-28：广告内容来源状态

`DesktopState::snapshot → AdRuntime::new → select_pins → content_source_configured`。状态使用同一个已选来源；`AdRuntime::snapshot` 单独负责可见缓存和错误。`DesktopState::refresh_ads → ad_refresh_waiting` 按等待原因生成恢复提示。来源已配置不代表已发布或已验签。[回归与边界](../acceptance/evidence/no-account-20261027/ad-source-status-20261028.md)。

CodeGraph 状态 complete，待处理变更为 0：306 文件、7,543 节点、30,065 条关系。静态图不代表运行验证。

## 2026-10-28：归档选择与口令交接

`PublisherRecoveryPanel / PublisherPage → PublisherFilePicker.withArchive → read → readFile → PublisherModel.importArchive / restoreBackup`。两个入口复用同一交接：取消或读取失败保留当前输入；非空文件读取成功后清空输入并提交。`readFile` 继续限制文件类型、链接和大小。[测试与边界](../acceptance/evidence/no-account-20261027/swiftui-archive-picker-20261028.md)。

本次 CodeGraph 状态 complete、pending 0：306 文件、7533 节点、30020 条边。索引统计不代表测试覆盖率。

## 2026-10-28：远程发布请求隐私

`Publisher::publish_configured_with → R2Store / AWS S3 SDK → LocalS3`；同一模拟服务记录请求路径、请求头与请求体。接收侧经 `ContentSource → AdCache` 验签和读取素材。测试先验证捕获器能观察查询参数、请求头和正文，再检查合成账号、代理和私钥标记。[测试与边界](../acceptance/evidence/no-account-20261027/r2-publication-privacy-wire-20261028.md)。

本次 CodeGraph 状态 complete、pending 0：306 文件、7528 节点、30008 条边。索引统计不代表测试覆盖率。

## 2026-10-28：活动记录并发保护

`DesktopState::activity / log → ActivityLogLock → LifecycleLock::try_named → activity.json`。两版使用各自发行目录内的锁。写入在锁内读取、脱敏并追加；读取已有记录也先取锁。锁占用超过两秒返回可重试错误。[测试与边界](../acceptance/evidence/no-account-20261027/activity-concurrency-20261028.md)。

本次 CodeGraph 状态 complete、pending 0：306 文件、7526 节点、29988 条边。索引统计不代表测试覆盖率。

## 2026-10-28：SwiftUI 快照乱序保护

`SwitcherWorkspaceView` 定时调用 `WorkspaceModel.refresh → DesktopBridge → workspace_snapshot`。`WorkspaceModel` 用快照代次丢弃较早的异步读取；通过 `beginAction` 启动的操作也会使先前快照失效。检查报告继续由独立的检查代次保护。[测试与边界](../acceptance/evidence/no-account-20261027/swiftui-snapshot-order-20261028.md)。

本次 CodeGraph 状态 complete、pending 0：303 文件、7471 节点、29751 条边。索引统计不代表测试覆盖率。

## 2026-10-28：正式渠道备份恢复与远程续发

`Publisher::restore_backup → Publisher::open → publish_configured_with → R2Store / AWS SDK → 回环模拟对象存储 → ContentSource / AdCache`。恢复不自动发布；显式续发先验证远程 current，再补传缺失素材、条件写入新版本并匿名回读。接收端验签后采用版本。[测试和边界](../acceptance/evidence/no-account-20261027/r2-backup-resume-20261028.md)。

本次 CodeGraph 状态 complete、pending 0：303 文件、7468 节点、29730 条边。索引统计不代表测试覆盖率。

## 2026-10-28：SwiftUI 产品入口与工作区提示

产品构建启用 `native-ui`：`SwitcherWorkspaceView → WorkspaceModel / PublisherModel → DesktopBridge → TauriWebViewTransport → WKWebView → Tauri IPC → Rust`。未启用该特性的开发构建仍使用 `App.svelte`。工作区的 `beginAction` 统一清理检查、普通操作、设置保存、记录导出和商家链接核验开始时的旧提示；三项新增回归验证失败与重试。

启动路径为 `DesktopState::launch_checked → lifecycle::launch_checked → launch_expected_with → 完整检查 → 两次启动决定复核 → open`。证据超过时限或网络代次变化会阻止启动，提示重新检查。既有边界测试没有在此轮重复运行。

本次 CodeGraph 状态 complete、pending 0：303 文件、7465 节点、29686 条边。索引统计不代表测试覆盖率。[验证记录](../acceptance/evidence/no-account-20261027/swiftui-workspace-feedback-20261028.md)。

## 2026-10-27：SwiftUI 基础与隔离桥接（历史）

当时产品仍从 `App.svelte → Tauri IPC → Rust` 启动。`SwitcherPreview → SwitcherShell / OverviewView` 只显示合成数据；原生宿主及其他页面当时尚未接入。[历史验证范围](../../output/no-account-20261027/swiftui-foundation/README.md)。

当时 CodeGraph 状态 complete、pending 0：283 文件、6985 节点、28337 条边。

## 2026-10-27：恢复确认后的界面锁

`PublisherRecovery.confirm → publisher_recovery_confirm → refreshAfterConfirm → Operations.reloadPublisher → ad_draft_read / publisher_status`。读取完成后才释放操作锁；读取失败时保留锁并由 `retryRefresh` 重试。[真实窗口证据](../../output/no-account-20261027/native-recovery-ui/README.md)。

CodeGraph complete、pending 0：270 文件、6761 节点、27916 条边。窗口测试使用合成密钥库和本机对象存储。

## 2026-10-27：远程结果未知的跨进程恢复

`写入测试进程 → Publisher → R2Store / AWS SDK → 本机模拟存储 → 持久化待处理状态 → 恢复测试进程 → 公开回读与释放锁`。写入进程在记录结果未知后被终止；新进程恢复同一签名版本。[证据](../../output/no-account-20261027/r2-process-restart/README.md)。

CodeGraph complete、pending 0：265 文件、6710 节点、27799 条边。完整桌面 App、默认 Keychain 和真实服务仍待验。

## 2026-10-27：保留代次的读取与验证

`Publisher::retained_views → PrivateDirectory::child/read_archive_file → 状态哈希与严格 JSON → cleanup::validate_batch / validate_state → RetainedView`。缺失或损坏时保持未验证状态；仅整理等价条件并补充行为测试。[证据](../../output/no-account-20261027/storage-final-verify/current/README.md)。

CodeGraph complete、pending 0：265 文件、6703 节点、27764 条边。

## 2026-10-27：产品页面远程配置接线

`RemotePublisher.svelte → 测试构建专用 invoke 包装 → 正式 Tauri IPC/权限 → Publisher 远程配置 → 合成 MemoryVault`。测试包装只模拟启动时的工作区快照和环境检查。产品页面的预览/正式保存、本机切换和旧目标选回通过。[证据](../../output/no-account-20261027/native-remote-ui/README.md)。

CodeGraph complete、pending 0：265 文件、6703 节点、27763 条边。默认 Keychain、真实 R2 与线上发布仍待验。


## 2026-10-27：远程配置原生 IPC

`测试前端 → WKWebView → publisher_remote_read/save/select → Publisher::configure_r2 / select_retained_r2 → Registry → 合成 MemoryVault`。13 次响应覆盖渠道隔离、目标保留和切回本机；5 个检查点未产生公开内容。[证据](../../output/no-account-20261027/native-remote-config/README.md)。

测试没有点击产品 Svelte 页面，也未使用默认 Keychain 或真实 R2。CodeGraph complete、pending 0：259 文件、6668 节点、27694 条边。

## 2026-10-27：远程写入结果未知的恢复

`publish_configured_with → remote_lock → Publisher::publish_with → R2Store / AWS SDK → 模拟存储`。模拟端保存 current 后断连；重新打开 Publisher 后，现有恢复路径核对同一版本并公开回读，最后释放锁。[证据](../../output/no-account-20261027/r2-uncertain/README.md)。

CodeGraph complete、pending 0：256 文件、6641 节点、27624 条边。这个测试在同一进程内重开对象，不证明应用进程重启或真实 R2。

## 2026-10-27：旧测试意图导航

CLI 账号切换：`Invocation::valid_business_arguments → lifecycle::switch_checked → switch_with → launch_expected_with`。首次检查失败不切换；切换后重新检查，再决定能否启动。诊断由 `network` 和 `diagnostics` 负责；无 TUN 写入门禁由 helper 的 `platform` 执行。21 项旧测试到现行测试的导航见 [F-01 映射](../acceptance/evidence/no-account-20261027/f01-intent-map.md)。

## 2026-10-27：广告失败退避时间修复

`AdRuntime::refresh_if_due → refresh_locked → RefreshSchedule::complete_failure`。失败后按结束时钟安排重试；结束时钟不可用时按已用单调时间估算。相关运行时 38 项通过。[证据](../../output/no-account-20261027/ad-scheduled-recovery/README.md)。

CodeGraph complete、pending 0：256 文件、6639 节点、27601 条边。真实休眠和线上传播未验证。

## 2026-10-27：发布内容与接收端组件联通

`Publisher → R2Store / AWS SDK → 回环模拟存储 → ContentSource → AdCache → 图片校验`。现有集成测试增加接收端路径、两次版本采用、旧版本与签名篡改拒绝。测试用本机 HTTP 重定向模拟源，不运行生产 HTTPS 客户端或后台计时器。[证据](../../output/no-account-20261027/r2-recipient-integration/README.md)。

普通桌面启动会调用 `refresh_ads(false)`，之后每 30 秒再次判断；实际刷新周期由 `RefreshSchedule` 控制。这是代码路径核对，不是在线传播验收。CodeGraph complete、pending 0：256 文件、6633 节点、27554 条边。

## 2026-10-27：R2 SDK 与发布流程的本机联通

`publish_configured_with → 目标选择 / 发布锁 → Publisher → R2Store → AWS S3 SDK → 回环模拟对象存储`。测试复用现有合成配置和密钥库。公开读取、图片与签名版本、条件写入和冲突保留由同一场景验证。模拟服务器和延迟发送回归只在测试中使用；没有变更产品运行逻辑或依赖。[证据](../../output/no-account-20261027/r2-sdk-integration/README.md)。

CodeGraph complete、pending 0：256 文件、6632 节点、27535 条边。此测试不证明真实 TLS、Cloudflare 服务端签名校验或客户端传播。

## 2026-10-27：单槽暂停与历史恢复

native-pause-history/runner.py → native-publisher 隔离工具 → WKWebView → 正式 IPC → Publisher::pause / restore_history / publish。独立前端生成三槽六期，覆盖单槽与全暂停、未知历史拒绝和恢复后显式发布。宿主检查 8 个磁盘快照，并独立验证 4 个正式版本的 Ed25519 签名及递增序号。产品代码和依赖未改。[证据](../../output/no-account-20261027/native-pause-history/README.md)。

CodeGraph complete、pending 0：255 文件、6616 节点、27469 条边。真实产品前端、默认 Keychain、OS 重启、R2/CDN 与 AD-07 全部边界未因此通过。

## 2026-10-27：WebView 恢复会话确认和取消

`native-recovery/runner.py → native-publisher 隔离工具 → WKWebView → 正式 IPC 注册表 → RecoverySessions / PreparedRecovery`。14 次响应与 6 个检查点验证取消、重复准备和错误确认的拒绝，以及正确确认后的旧代次/备份保留、无自动发布和显式续发。宿主独立验签。测试前端、MemoryVault 和本机对象存储均为合成夹具，真实产品 UI 对话框仍待验。[证据](../../output/no-account-20261027/native-recovery/README.md)。产品代码未改；相关旧测试与 ACL 证据复用。CodeGraph complete、pending 0：252 文件、6581 节点、27393 条边。

## 2026-10-27：真实 WebView 经正式注册表完成运营闭环

`ipc-business --ordinary-publisher → ordinary::run → 正式 ipc::handler / capability → WKWebView invoke → Publisher`。独立测试前端调用正式命令；仅测试特性允许注入 MemoryVault。默认产品启动与 Keychain 路径保持原实现。显式启用参数和私有合成目录是测试入口的前置条件。

原生窗口完成 17 次响应和 5 个宿主检查点：创建密钥与重复拒绝、素材规范化、三槽六期、预览/生产签名发布、加密备份、较高暂停版本、错误口令拒绝、恢复且不自动发布、显式更高序号续发。独立 Node Ed25519 验签，并核对素材摘要/尺寸/0600、current 与 immutable、旧代次保留和恢复状态。

测试工具首次漏建合成目录，窗口创建前退出；已修复并补回归。复用成功构建，保留失败证据；成功原生调用不重复执行。相关 IPC 7 项和工具 8 项通过。Personal 测试特性 Clippy、Public 测试特性编译及四文件格式通过；Clippy 保留三条既有迁移风格警告。完整 236 项与 ACL 142 次拒绝沿用旧有效证据。CodeGraph complete、pending 0：249 文件、6543 节点、27309 条边。[计划](../plans/WebView正式运营成功路径-2026-10-27.md)与[证据](../../output/no-account-20261027/native-publisher/README.md)。

范围是正式处理函数、真实 WKWebView、测试前端和合成密钥库。默认 Keychain 权限、普通产品前端、R2/CDN、原生两次确认及 AD-10 整项仍待验。UI 最后，保留当前布局和非蓝紫色背景；注册、部署、付款暂停。

## 2026-10-27：私有 Keychain 的跨进程恢复通过

`native_keychain_process_tests 协调器 → 写入 worker → wait 回收 → 新读 worker → Publisher::open / KeychainVault::isolated`。两个 worker 使用同一编译测试二进制、不同 OS 进程；写入者退出后才启动读者。读者验证恢复代次、recovery_required、旧密钥与恢复密钥、旧状态摘要及无自动发布。协调器按精确清单清理条目，删除私有库，核对偏好快照。

一个原生验收测试通过，内部两次 worker 不累计为独立功能数；新增两个入口默认忽略，缺少显式启用提前拒绝。权限 fixture 首次失败已修正，保留严格断言与 RED/GREEN。只扩展测试模块，产品及依赖未改；定向编译、Clippy、格式通过，复用上一阶段 236 项完整测试。CodeGraph complete、pending 0：243 文件、6433 节点、27096 条边。[证据](../../output/no-account-20261027/native-keychain-process/README.md)。不外推为默认库权限、WebView 成功、整机重启、实源或完整 AD-10。UI 最后，沿用当前布局及非蓝紫色背景；注册、部署、付款暂停。

## 2026-10-27：独立 Keychain 内原生恢复通过

`ManagedVault::default → KeychainVault::default` 保留默认库选择。KeychainVault 仅在测试构造器中接受私有 Keychain，前端无选择入口。创建仍走同一 SecItemAdd，服务名和编码函数集中复用；私有库读写用 SDK 显式限定位置，默认库 SDK 路径保持原样。

`native_keychain_tests → 私有库 A/B → KeychainVault → Publisher` 已通过一个原生测试：跨库同名条目隔离、重复创建保护、真实双线程竞争、无效资料拒绝、备份/错误口令/恢复/旧代次与旧密钥保留、同进程重开。清理后条目清单为空、两个私有库已删除；受限进程前后偏好快照相同。Personal 236 项通过、原生用例默认忽略 1 项；Personal 全目标 Clippy、Public 全目标编译及两文件格式通过。CodeGraph complete、pending 0：242 文件、6415 节点、26991 条边。[证据](../../output/no-account-20261027/native-keychain-isolation/README.md)。

前阶段默认库 -25307 的失败证据保留。此处通过的是明确指定的独立库，不升级默认库权限、OS 进程重启、成功 WebView、实源或 AD-10 整项。

## 2026-10-27：原生 Keychain 验证入口与阻断证据

`security.rs → cfg(test, macOS) native_keychain_tests → OwnedVault → 正式 KeychainVault / Publisher`。测试默认忽略，必须显式启用并使用全新合成目录。OwnedVault 只允许更新本次成功创建的随机条目；竞争线程在所有权锁之外调用正式创建接口。正常与 panic 路径清理精确条目，并回查缺失。

本轮编译、Personal bin/tests Clippy、两文件格式及两项启用保护通过。原生尝试首次创建返回 SecretUnavailable；只读默认 Keychain 查询返回 -25307，正向恢复未进入，所有权清单为空。原生恢复保持 WAIT，不升级 AD-10。没有调整系统默认密钥库或搜索列表。[证据与范围](../../output/no-account-20261027/native-keychain/README.md)。CodeGraph complete、pending 0：242 文件、6403 节点、26914 条边。

## 2026-10-27：真实 WKWebView 拒绝边界补证

`native-ipc-policy/run.py → 普通 main/ipc::handler → 实际 capability → WKWebView 报告 → 宿主独立校验` 已完成两版远程来源主窗口和其他窗口矩阵。Public 23×2、Personal 48×2，共 142 条明确 ACL 拒绝。生产调用链和权限未改。

测试策略只额外允许 WebKit 必需的 `/private/var/run/mDNSResponder`；回归同时检查其他套接字、外网和真实资料仍拒绝。校验器支持本地根 URL 的两种序列化，拒绝来源伪装。两项缺陷均保留 RED/GREEN，完整 Python 集合 14 项通过。Public 只重校验已保存的原生回执，Personal 一次新构建取得回执。[输入与限制](../../output/no-account-20261027/native-window-unlocked/README.md)。真实 Keychain、成功业务与系统验收未因此通过。

## 2026-10-27：生产运营 IPC 共享密钥边界与成功恢复闭环

Personal 的 Tauri Builder 单独注册 ManagedVault，默认使用 KeychainVault。15 个需要凭据的生产命令经 Tauri State 注入同一服务，复用 Publisher→Vault 锁顺序；DesktopState 保持独立。前端 JSON、命令名、ACL 和 Public 运营隔离不变。合成 Vault 仅测试可构造。

新增两项共享产品分发表测试覆盖：密钥生成与重复拒绝、三槽六期、两渠道发布、备份导出与旧备份恢复、较高序号续发；恢复准备/取消/确认、旧代次导出与另一隔离安装导入。独立核对签名、immutable/current、规范化素材、0600 文件、旧代次/旧密钥和当前状态；恢复与归档导入均不自动发布。另补 Vault 锁中毒后失败关闭测试。

[计划](../plans/生产运营IPC密钥边界与备份闭环-2026-10-27.md)与[证据](../../output/no-account-20261027/publisher-vault-dispatch/README.md)。阶段末 Personal 主程序 236 项、Public 主程序 35 项通过；两版四个集成目标各 140 次通过，同一专用 Seatbelt 用例各按原配置忽略一次，不计为通过。两版 all-target Clippy 和六个变更文件格式检查通过，保留三条既有 Personal 迁移风格警告及 ts-rs 提示。294 个源码/构建/测试输入在完整验证后无漂移。CodeGraph complete、pending 0：241 文件、6373 节点、26801 条边。本轮仍是 MockRuntime/MemoryVault；真实 Keychain、WebView、R2 传播和系统验收继续待验。UI 最后执行，沿用当前布局和非蓝紫色配色；注册、部署、付款暂停。

## 2026-10-27：恢复拒绝的共享注册表测试

`ipc::handler → restore_publisher_backup → spawn_blocking → Publisher::restore_backup` 与 `ipc::handler → publisher_archive_import → spawn_blocking → import_retained_generation` 新增同一隔离测试。两入口各八类拒绝，独立递归核对磁盘状态并重建 App 回读；产品调用链未改。[证据与 MockRuntime 范围](../../output/no-account-20261027/recovery-dispatch/README.md)。CodeGraph complete、pending 0：240 文件、6350 节点、26499 条边。

## 2026-10-27：发布反馈的目标范围修正

`publisher_status → publisher_view` 的未完成发布提示改为所选发布目标；`restore_publisher_backup → restore_publisher_backup_body` 明确本地恢复尚未发布。调用链与上传/恢复事务不变。新增两项反馈回归，发布命令单元集合 10 项通过；[证据](../../output/no-account-20261027/publication-feedback/README.md)。CodeGraph complete、pending 0：240 文件、6347 节点、26460 条边。

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

## 2026-10-27：F-05 独立卷与真实空间不足路径

新增 `accounts/tests/native_metadata/volume.rs` 与 `tests/native_accounts/` 运行器。调用生产 `select_legacy_checked → legacy::select → ditto → Darwin 目录交换`；active 或存档放入独立 APFS 卷，覆盖两个方向的最新 A/B 往返与目标空间不足拒绝提交。共用父模块的元数据读取与树断言，未改生产业务。

有效结果为 `r3` 三项与 `r6` 一项，按源码片段及哈希复用，不称为一次全绿运行。运行器两项单元测试、最终 Clippy 与格式检查通过。失败夹具记录、源码绑定和卸载证明见[阶段证据](../../output/no-account-20261027/f05-volume-copy/README.md)。CodeGraph complete、pending 0：238 文件、6321 节点、26370 条边。

完整 F-05 分支、普通 App 动态 IPC 及其他系统验收仍待完成。下一步继续前序核心/IPC 缺口；UI 重写仍最后执行，沿用当前布局。注册、部署、付款继续暂停。

## 2026-10-27：生产账号路径的原生元数据测试

新增 `accounts/tests/native_metadata.rs`：`select_legacy_checked → accounts::legacy::select → ditto → Darwin 目录交换 → 状态/current/清理`。只替换进程观察；APFS、复制、交换和 xattr 均真实执行。测试独立读取嵌套树元数据，覆盖 A/B 最新数据往返与仍运行时不提交。

修复 `.gitignore` 将账号源码视为资料的规则，保留资料目录忽略并补 RED/GREEN 回归。CodeGraph 已增量同步：235 文件、6282 节点、26222 条边，complete、pending 0。核心测试首次 441/451 通过，修正测试运行环境后仅重跑失败的 10 项，全部通过；Clippy 通过。[执行证据](../../output/no-account-20261027/f05-native-metadata/README.md)。完整原生/跨卷验收仍未完成。

## 2026-10-27：远程来源与其他窗口测试工具

已补测试窗口与本机服务器，保留普通 Rust 后端和实际权限。Public/Personal 计划逐上下文拒绝 23/48 个命令，校验完整序列、版本、URL 和真实 IPC 桥；缺桥、缺参或未知命令不能计 ACL 通过。复用同一报告读取与隔离生命周期，不新增产品测试命令。

12 项 Python 测试、9 项 Node 测试通过。修复测试客户端继承代理导致本机请求 502，并补失效代理、异常关闭及端口复用回归。Public 构建成功，两次运行未取得回执；系统随后确认 Mac 锁屏。两次直接进程均已停止。动态状态仍待验，Personal 未启动边界构建；不能归因所有历史失败或升级完整矩阵验收。[证据](../../output/no-account-20261027/ordinary-ipc-boundaries/README.md)。

产品 UI、Rust 业务、权限和系统配置未改；注册、部署、付款继续暂停。原生 SwiftUI 玻璃效果重写仍排最后，布局保留当前产品布局。


## 2026-10-27：普通后端真实 WebView IPC

新增 `tests/native-ipc-policy`：普通 Rust main、命令处理函数、两版 capability/CSP → 测试 frontendDist → 真实 WKWebView IPC → 宿主读取页面 JSON → 独立核对合成 HOME 文件。不添加产品命令或 fixture feature；测试 App 包装与测试前端均单独标明。

两版共 14 次业务成功、21 次缺参拒绝、33 次 ACL 拒绝（含 Public 全部 25 个运营命令）。共 68 条响应，不作为 68 个独立测试名。7 项 Python 校验测试和 1 项 Node 报告回归通过。修复夹具中间目录 755 问题并补回归。[阶段证据](../../output/no-account-20261027/ordinary-ipc-runtime/README.md)保留未成功尝试、有效回执和源码绑定。

远程来源、其他窗口及全部业务成功路径的动态矩阵仍未完成；不升级为完整普通 App 验收。CodeGraph complete、pending 0：232 文件、6164 节点、25338 条边。产品 UI 布局未改，原生 SwiftUI 玻璃效果重写仍排最后。注册、部署、付款保持暂停。

## 2026-10-27：普通发行 IPC 权限策略

`main.rs` 的普通测试入口 → `ipc_policy_tests.rs` → 构建生成 manifest/capability + 两版实际配置 → Tauri `Resolved::resolve` → `RuntimeAuthority::resolve_access`。无需增加依赖或产品测试后门；夹具 feature 排除本模块。

2 个测试名在两版执行共 4 次通过。Personal 48、Public 23 个授权与注册表一致；全部授权限定本地 main 窗口。覆盖远程来源、其他窗口、夹具/未知/插件命令拒绝，以及 Public 拒绝 25 个运营命令。两版 Clippy 和格式检查通过，保留既有警告。[证据](../../output/no-account-20261027/ordinary-ipc-policy/README.md)明确区分策略测试与未完成的普通 App 动态 IPC 验收。

CodeGraph 自动同步为 complete、pending 0：228 文件、6110 节点、25110 条边。UI 布局未改。原生 SwiftUI 重写仍排最后，保留当前布局，只参考图片玻璃效果，沿用非蓝紫色配色。

## 2026-10-27：R2 IP 字面量拒绝与发布说明修复

`publisher_remote_save` → `configure_r2` → `R2Config::validate`；同一校验也用于目标身份计算及 `R2Store::open`。原字符串 IP 解析漏掉带方括号的 IPv6。现改用 URL 解析器的域名类型，统一拒绝 IPv4/IPv6 字面量，不新增依赖或重复校验入口。

`Operations.svelte` → `publish_ad` → `publish_configured` 的调用链不变。发布卡片说明按渠道选择 R2 或本机目录；历史标签不再把远程发布标为本机发布。现有布局、导航和按钮位置不变。Svelte 回归实际编译并渲染生产发布及历史片段。

两个缺陷均保留有效 RED，修复与检查见[本阶段证据](../../output/no-account-20261027/r2-origin-ip-literals/README.md)。CodeGraph 自动同步完成：227 文件、6103 节点、25094 条边，complete、pending 0。普通 App 证据仍绑定 9d6 源码，本阶段没有重复打包或重写 SwiftUI。

阶段末检查：桌面后端 224 项、前端 64 项及两组 VM 回归通过；Svelte 检查 0 错误/0 警告，两版前端构建、Public 编译和格式检查通过。Clippy exit 0，保留 3 条既有迁移风格警告。R2 定向 11 项已包含在完整套件内，不累加。

## 2026-10-27：普通 App 实际业务路径与后续 UI 计划

普通发行入口 → 现有 Tauri/WKWebView → 设置、活动记录及 Personal 运营页面 → 正常业务命令 → 合成 HOME 内持久文件。源码基线为 `9d6a3efcfc6e907ae7797633a843720533a94b34`；未启用 fixture feature，未注入测试 WebView。两版实际验证偏好保存、重启和脱敏导出；Personal 另验证草稿保存、重启及发布来源读取。回执见[ordinary-app-r1](../../output/no-account-20261027/ordinary-app-r1/receipt.json)。

Public 无运营导航只属于界面证据；完整动态 ACL 命令矩阵仍未证明。R2 表单读取不证明真实 Vault 写入或远端发布。隔离环境的 14 项检查仍有 13 项阻塞。真实 Keychain/R2、安装、传播和系统托管 WebKit 回收仍未验收。

产品源码未变。当前索引状态复核为 complete、pending 0：227 文件、6100 节点、25090 条边。复用既有图谱，不重复重建。后续原生 SwiftUI 流体玻璃重写排在任务清单最后，保留当前 UI 布局，采用非蓝紫色配色；详见[设计约束](SwiftUI流体玻璃重写-排期与设计约束-2026-10-27.md)。本轮没有把 Tauri/Svelte 改为 SwiftUI。

## 历史阶段：远程锁与全部目标清理接线

`publisher_commands::with_cleanup_source` → `Publisher::with_configured_currents` → `connection/maintenance.rs::Sources` → 既有 `cleanup_preview/commit`、`gc_preview/commit`。先核对全部账本的配置覆盖及本机目录身份，再按物理锁标识排序取得全部远程锁。一个 `LocalObjectStore` 覆盖本机两渠道；同一 R2 桶、账号与渠道的不同公开域名只取得一把锁。每轮引用读取先复核锁，清理算法继续验证签名、版本水位和预览摘要。

```text
渠道配置表 + 全部目标账本 → 完整来源覆盖 → 本机锁 + 远程 CAS 锁
                                                      ↓
认证全部 current → 索引整理确认 → 独立文件回收确认 → 释放锁
                                                      ↑
                        发布结果未知：保留锁，重启后沿用所有者回查
```

`publish_configured` → `remote_lock::Lease::acquire` → 共用发布事务 → `Lease::release`。`r2_store.rs` 只为固定的渠道锁路径开放内部认证 GET/条件 PUT；普通素材、版本和公开读取不能操作锁。锁状态包含随机所有者与随机版本；不自动到期，不抢占其他所有者，不删除锁对象。每次重取锁先执行新 CAS，隔离旧请求。锁意图先持久写入根目录 `remote-locks.json`，不含凭据，也不进入可移植备份。未知写入和待回查发布保留意图；有意图时阻止备份恢复与有效代切换。

修复同桶更换公开域名后版本文件 ID 冲突：新操作避开全部目标历史中使用过的同渠道发布 ID。失败复现、最终验证和源码摘要见[远程锁回执](../../output/no-account-20261027/remote-locks/receipt.json)。测试覆盖锁竞争、迟到请求、重启、坏记录、部分取锁失败、来源缺失、域名去重、旧目标素材保留及真实隔离目录回收。

保证范围是采用该锁协议的合作发布器。旧客户端、手工写桶或丢失本机锁意图不属于自动协调保证；不按时间抢锁。真实 R2/Keychain、普通 App 动态 IPC 和远程传播仍未验收。注册、部署、付款继续暂停。

本阶段桌面后端完整套件 223 项通过，0 失败；其中相关连接测试最终 22 项通过，包含在 223 项内，不相加。完整套件后只合并两处条件语句，并针对最终代码复跑连接测试。Clippy、Public 编译和格式检查通过；保留 3 条原有迁移风格警告。前端未改动，沿用上一阶段有效构建记录。图谱为 227 文件、6100 节点、25090 条边，complete、pending 0。

## 历史阶段：保留发布目标与配置回切

`RemotePublisher.svelte` → Personal 专用 `publisher_remote_read/save/select` → `connection.rs` → `connection/registry.rs` → 现有 Vault。每个渠道保留独立目标配置表，目标标识必须与配置重新计算的身份一致。读取旧单目标格式时兼容转换；保存时写入新格式，不另建明文凭据文件。

更换目标或选择本机目录保留旧目标的访问资料。同一身份轮换凭据只替换该条目。选回保留目标复用 Vault 内的密钥，IPC 不返回密钥；已有待恢复事务继续阻止切走。跨渠道检查同时覆盖当前和保留目标。每渠道最多 16 项，总编码仍受原有 16 KiB 凭据上限约束；超限拒绝保存，不自动清除旧目标。

```text
配置保存／目标回切 → 校验目标表 → Vault 原子保存
                                   ↓
发布 → 选中目标 → 原目标账本 → 共用发布事务
```

响应包含无秘密的目标列表和缺失访问资料的旧账本标识。已有发布记录但没有旧配置时，操作员须重新保存原配置才能恢复覆盖。该列表只证明配置覆盖，不证明远端可达或已经加锁。产品中的远程多目标清理仍未开放，远程锁仍需实现。

更换配置丢失旧目标的 bug 已有 RED 与 GREEN。重启、加密备份、同目标轮换、错误渠道、坏记录、容量及失败保留的证据见[目标配置回执](../../output/no-account-20261027/target-registry/receipt.json)。普通 App 动态 IPC、真实 Keychain/R2、正式发布与传播仍未验证。本轮不注册、不部署、不付款。

本阶段发布器完整套件 174 项通过，包含 14 项配置测试，不重复计数。两版前端构建、Public 编译、Svelte（0 错误/0 警告）、格式检查通过。最终 Clippy 退出码 0，保留 3 条原有迁移代码风格警告。图谱为 223 文件、6009 节点、24577 条边，complete、pending 0。

## 历史阶段：旧发布资料恢复产品调用链

`PublisherRecovery.svelte` → Personal 专用 `publisher_recovery_commands.rs` → `PreparedRecovery`。后端会话跨准备与确认持有 Publisher 和本机发布树的生命周期锁。`RecoverySessions` 的取消代号阻止已取消的后台准备再次发布确认状态。Public 不注册或授权这六个命令。

`migration/recovery.rs` 负责完整归档、加密备份写入与读回核对。`migration.rs` 复用安全目录读取和归档内容校验，保留 state、checkpoint、assets、cleanup-gc、未索引文件及 Vault；完整归档另含 trust 和路径、类型、长度、摘要清单。准备后及原子切换前重新核对源资料、目录身份、凭据、备份和两渠道签名 current。取消或异常释放会话，保留备份、旧目录和旧密钥。

```text
旧资料 → 完整加密归档 → 读回核对 → 用户确认
                                      ↓
                    再核对 → 新代次 → 原子更新 active.json
                                      ↓
                       核对新草稿 → 用户明确发布
```

新代次只带草稿所需素材，序号高于全部目标水位；旧目标账本不自动归属新目标。完整归档导入只写入未激活的保留代次。保留资料可以读取索引并重新加密导出。恢复步骤没有远程 CAS；之后的明确发布仍走原共用发布事务。

回归覆盖旧 JSON 省略默认字段、原始 state 字节保留、完整清单、未索引/隔离文件、取消与锁、目录替换、来源和凭据变化、密文/口令、同步失败、最后时刻变更、密钥独占创建失败、切换结果未知、素材缺失/损坏及归档大小上限。验证结果和输入绑定见[恢复回执](../../output/no-account-20261027/publisher-recovery/receipt.json)。真实 R2、Keychain 和普通 App 动态 IPC 仍未验证；远程多目标清理仍未实现。注册、部署和付款未执行。

## 历史阶段：远程发布产品调用链

`Operations.svelte` → `RemotePublisher.svelte` → Personal 专用 `publisher_remote_read/save` → `Publisher::configure_r2` → 现有 Vault。读取响应不含密钥；保存配置不访问网络。

`publish_ad` → `Publisher::publish_configured` → 按渠道读取 `Connection` → `select_target` → `R2Store` 或 `LocalObjectStore` → 共用签名发布事务。`connection.rs` 负责配置校验、渠道隔离、凭据轮换和目标选择；`r2_store.rs` 负责 SDK 传输及匿名回读。`TargetLedger` 隔离各目标的历史、水位和未完成事务。

`connection/tests.rs` 使用合成 Vault 与真实隔离文件存储覆盖配置、失败保留、渠道选择、重开、轮换和加密备份。`tests.rs::target_ledgers_publish_remote_and_resume_without_replaying_into_local` 覆盖远程发布恢复、禁止未选择目标以及跨目标事务隔离。R2 SDK 协议替身测试随桌面套件集中执行。Public 不注册或授权这两个配置命令。

本阶段桌面套件首次为 186 通过、1 失败。失败仅来自新测试的提交尝试计数断言；修正后定向 1 项通过，累计 187 个不同测试名通过，没有重跑其余 186 项。新增配置 7 项已包含在桌面结果内。前端相关 27 项、两版前端构建、Public 编译、格式检查通过；Svelte 检查为 0 错误、0 警告。默认与 ipc-business Clippy 退出码均为 0，仍有旧清理/迁移代码警告，未宣称严格零警告。图谱为 218 文件、5879 节点、23801 条边，complete、pending 0。

[本阶段回执](../../output/no-account-20261027/remote-wiring/receipt.json)记录集中验证和输入摘要。[图谱状态](../../output/no-account-20261027/remote-wiring/codegraph-status.json)记录增量同步。图谱数量不代表测试覆盖率。真实服务、Keychain、普通 App 动态 IPC 和线上传播未验；旧状态完整迁移和远程多目标清理仍未完成。

阶段发布器套件 166 项通过。随后抽取共用归档内容校验并补齐缺失/损坏素材保护，最终归档与迁移测试 18 项、素材保护测试 1 项通过；这些计数有重叠，不相加。前端相关 24 项、两版前端构建、Public 编译与格式检查通过，Svelte 为 0 错误/0 警告。最终 Clippy 退出码 0，保留 3 条原有嵌套条件风格警告。图谱为 222 文件、5973 节点、24263 条边，complete、pending 0。

以下记录保留各历史阶段的证据和限制；“尚未接线”等描述仅适用于对应历史阶段。

## 历史阶段：密钥恢复验证与本地提交

密钥已写入而信任文件缺失时，恢复仍须保护现有凭据。仅明确的密钥不存在结果可视为空；读取失败须中止恢复。[验证回执](../../output/no-account-20261027/r2-target-identity/orphan-vault-summary.json)复用 9 项回归和 141 项发布器测试通过日志，本轮只补 Clippy（退出码 0，7 条未使用代码警告）和图谱同步。真实 Keychain、远程 R2 及产品接线仍未验证。此提交保存当前本地实现，不表示远程发布功能完成。

## 历史阶段：R2 发布目标标识

`publisher/r2_store.rs` 从账号、存储桶、公开域名和渠道生成目标标识，凭据不参与计算。`publisher/mod.rs` 仍拒绝远程目标；产品接线及真实远端验证尚未完成。[隔离测试摘要](../../output/no-account-20261027/r2-target-identity/summary.json)为 8 项通过，并记录本机原始日志的 SHA256。代码仍在未提交工作树中。

`publisher/mod.rs` 在跨渠道未完成事务和备份版本冲突时停止；[发布器回归摘要](../../output/no-account-20261027/r2-target-identity/publisher-summary.json)记录 75 项通过。代码仍在未提交工作树中。

工作树的 `TargetLedger` 为每个目标保存发布历史、水位和未完成事务，并保留共享草稿和素材。`publisher/target_backup.rs` 合并目标历史和水位；冲突时保留原状态。恢复后各目标需要重新核对。多目标备份的 7 项隔离回归通过，[测试摘要](../../output/no-account-20261027/r2-target-identity/backup-merge-summary.json)记录范围。`select_target` 尚无产品入口；多目标清理仍拒绝，远程发布仍被门禁阻止。

`publisher/security.rs` 在恢复前区分密钥不存在和读取失败。即使信任文件尚未写入，已有密钥也不会被恢复静默替换。该边界已在合成密钥库中复现并修复；9 项多目标备份测试通过。真实 Keychain 权限和错误行为尚未实测。

`publisher/cleanup.rs` 的引用计算保护其他目标的历史、待发布、恢复证据及经过验证的当前版本所引用的素材。多目标接口要求来源覆盖全部目标并在操作期间持有所有发布锁。`publisher/local_store.rs` 的 `MultiLocalSources` 可持有多个真实本地目录锁，并重新核对各目录身份；隔离测试覆盖目录替换、锁竞争和垃圾回收。产品命令尚未接入多目标来源，远程目标也尚无多目标锁方案；产品中的多目标清理门禁保持关闭。

本轮 CodeGraph 同步后为 214 文件、5761 节点、23121 条边，索引完成且无待处理变更。

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

历史本地发布阶段只装配 LocalObjectStore。本轮 publish_ad 已接入按渠道配置选择 R2Store；本机协议替身验证与真实 R2 验收仍分开记录。

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
