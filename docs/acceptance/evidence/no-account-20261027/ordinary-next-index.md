# 本轮独立证据索引：ordinary-next

解析器提交为 `ac1d48b49a72df6d83b7e08c05fc6406b3f7e2b8`，tree：`cbe31f21bd652991ada53c26a7507bec382d32d4`。`tests/evidence_reconciliation/`的25项回归及[独立审核](../../../../output/no-account-20261027/ordinary-next/review-rust-r2/portable-review.json)限定通过；仅支持当前串行libtest/F06三子进程输出格式，不支持任意并发或新嵌套格式。历史回放严格核对704顶层/600名，原失败summary保留；解析器核验证据一致性，不认证回执或重跑Rust。普通包仍绑定产品提交00dbf3c，不改标为解析器提交。最终[图谱状态](../../../../output/no-account-20261027/ordinary-next/codegraph-status-final.json)为175文件/4814节点/18877关系，complete、pending0。

普通双包独立复核结论为PASS_STATIC_ARTIFACTS_WITH_PROOF_LIMITS：每版12个现存条目/7文件及24份命令日志哈希重新核对，commit/tree/archive均匹配。临时frontend dist已清理，本次不能独立重扫，仅保留原构建扫描回执绑定。普通App动态ACL、启动和安装仍未验；解析器已完成限定独立审核。见[普通包独立复核](../../../../output/no-account-20261027/ordinary-next/review-bundles/review.json)。

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

独立审查仅重放既有日志及完整性绑定，没有新执行测试。解析器已提交并通过25项限定回归及独立审核。两项原生测试继续排除。700/596保留为此前r1轮次，不与r2混计。普通双包已从本提交完成构建及限定静态检查，独立包复核已完成限定静态核对。产品提交阶段图谱172文件/4748节点/18696关系，complete，待更新及待解析均0；不表示覆盖率。

## 普通双包静态交付

绑定00dbf3c精确归档，archive SHA256为 `b8cd16b1166acbbcd2dae62ae7cc2e5eb52ddb86db02a766ec50610381f86172`。两版各22条静态检查及2条复制/签名变更完成；Public后每版12条目重验通过。business/limits运行体标记在dist/resources及四个Mach-O二进制未检出。历史E01权限描述forbidden_probe/finish_smoke各1处，保留REVIEW_MARKERS_PRESENT；静态排除不证明普通App动态ACL，后者仍NOT_RUN。独立包复核已完成限定静态核对；解析器已完成限定独立审核，普通启动、安装、真实账号等限制不变。

| 版本 | App路径 | 签名主程序SHA256 | 清单SHA256 |
| --- | --- | --- | --- |
| personal | `/Users/gua/CodexWorkspaces/claude-switcher/output/no-account-20261027/bundles-ordinary-next/artifacts/personal/Claude 切换器 Personal.app` | `5afb4f7dbdc97db933f39904fda6f2e44c6696c46546c9fd76da115a47437812` | `82ec45d704398cfe2ddf57a5a84e65951904833cabb7ec9d2baaa8c3a5b90a3c` |
| public | `/Users/gua/CodexWorkspaces/claude-switcher/output/no-account-20261027/bundles-ordinary-next/artifacts/public/Claude 切换器 Public.app` | `bd4db1a38c9fec0184beb55635ff068a0ac01da84e88486e295ac3dc98e6bd01` | `2f540ca60b72c3a2ed33d082ffad9437aa6ddabcc34e30381d49dc5670e03158` |

## 仍在推进

稳定runner r9已完成下方限定验证；最终Rust统一全量r2已完成四命令及独立分组复核。普通双包已从00dbf3c精确归档完成构建及限定静态检查。独立包复核已完成限定静态核对；解析器已完成限定独立审核；CNC历史包保留，不把fixture构建视为普通候选包。真实账号、正常App启动、实源、系统、安装和长时验收保持未验。既有33–66小时粗估不因有限推进缩减。

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
| 普通包独立复核 | [review.json](../../../../output/no-account-20261027/ordinary-next/review-bundles/review.json) | `f36ee5019caad91db144068d798abdc8778515b27aa42a655dff975520c336b7` |
| 解析器限定独立审核 | [portable-review.json](../../../../output/no-account-20261027/ordinary-next/review-rust-r2/portable-review.json) | `aba9b4c07e7e56b701ed7d3691ae390f3d3c052684cd994c87a0ec49bb73f6bb` |
| 最终图谱状态 | [codegraph-status-final.json](../../../../output/no-account-20261027/ordinary-next/codegraph-status-final.json) | `fc781327f16d2518f3ae993b0a6b43dee460cdd5397e30c0f604c7393adf0e0c` |

原生排除：`unregistered_or_wrong_signer_service_never_authenticates`、`time::tests::real_macos_continuous_sample_is_available`。
