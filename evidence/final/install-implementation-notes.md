# 安装与独立诊断旧行为恢复

1. 普通 install 默认替换同 owner/edition 组件；先对该 owner 的 Claude/切换器发送 TERM 并等退出，再获取事务锁。可选 --digest 保留显式预览；省略时内部绑定当前计划。
2. verify 在普通用户执行，不获取root事务锁、不退出应用、不读0600安装回执。仅验证公开安装记录、组件签名/权限及服务进程；结果明确 authenticated_health=not_run。
3. 普通安装以组件校验和服务进程验证完成，不启动GUI、不等待GUI/guardian认证回执。可恢复备份、组件一致性、旧服务接管仍保留。已有旧事务可继续严格认证验证。
4. 新事务不解析/消费/删除repair journal、system-original或repair receipt，不用其状态阻挡替换/卸载。旧版安装事务记录中的managed_system快照仍支持原有恢复验证。跨owner/edition迁移仍拒绝且不触碰用户资料。
5. 独立诊断配置接受未知字段、首尾空白、普通可读文件及符号链接。仍拒绝无效代理协议/端口/路径和URL内部空白，报告仍不包含凭据。

D02: helper XPC 的精确代码pin仍保留；/Applications可由admin组写入，不在该路径取消安全边界。普通源码核心的去登记依赖由主任务处理。本分支没有修改helper、domain或core/install.rs。
D12: 远程正式发行后置；SourceLocal自动更新仍禁用。

验证: python unittest packaging/macos 共55项通过；py_compile通过。Rust diagnostics测试尝试因并行regions/guardian接口未完成而暂时编译失败，见diagnostics-tests.txt，需集成后重跑。
未运行: sudo、安装、launchctl真实变更、真实App、账号或系统设置操作。

## D10 root时区守护恢复

安装器保留local.tz-watchdog旧服务，不再把它当作待删除服务。安装/替换从owner的v2/config.json timezone_target读取目标；没有v2配置时仅读取旧脚本TZ_TARGET字面量。目标必须通过字符/路径与/usr/share/zoneinfo存在检查；不执行用户脚本。

安装器将固定旧时区脚本逻辑写入root保护的SYSTEM/tz-watchdog.sh（755），固定plist写入/Library/LaunchDaemons/local.tz-watchdog.plist（644）。两者由root安装进程创建，默认root:wheel。脚本只调用固定systemsetup目标；3600秒周期和RunAtLoad保持旧语义。已有脚本必须root:wheel且权限正确；安装器不信任用户可写程序。安装事务备份旧脚本/plist，恢复原有注册状态；新建安装回滚会移除自己新建的脚本/plist。普通卸载保留原有独立root时区守护。

D10边界: 旧bodyguard由Rust guardian接管；SourceLocal旧12小时更新Agent仍停用。仅恢复旧root时区纠偏，不引入新服务类型。真实root运行、重启后周期纠偏尚未验收；等待最终统一安装。新增目标解析、拒绝注入/用户所有脚本、fresh创建回滚和服务注册失败恢复测试；Python共59项通过。

## D15 Rust CLI旧入口兼容

prepare-native.sh构建、签名并复制claude-switcher，Tauri资源表包含components/claude-switcher。SourceLocal plan检查CLI存在、可执行、签名identifier符合local.claudeswitcher.cli。App整体SHA与签名事务同时覆盖该CLI。

安装器以owner权限原子写入~/.local/bin/claude-switch.sh。入口只保存COUNTRY_TARGET、TZ_TARGET、LOCALE_TARGET、PROXY_MODE和PROXY_GROUP字面量，再exec固定App内Rust CLI（传入所属edition及原始参数）。使用单引号及固定转义保存$、空格和单引号；不执行旧shell主体。旧入口私有备份在安装事务目录，失败回滚恢复原内容/权限。新建入口失败回滚删除；用户在安装后自行修改入口时，恢复拒绝覆盖该修改。日常配置读取器需支持该单引号格式，已与主任务协调。

新增测试覆盖全部5项配置保存、旧入口恢复、新入口移除、预览后变更拒绝、含$与单引号组名、public edition和带空格参数透传。Python共63项通过；diagnostics Rust单元4项通过；bash -n、py_compile通过。实际App资源打包/管理员安装留待主任务统一执行；未执行任何真实安装或账号操作。

## D05 同owner发行替换

plan读取root安装记录校验owner与旧edition，检查旧App签名身份匹配旧record。迁移计划新增previous_edition、old_app旧状态和旧App固定目的路径。安装事务先备份全部组件，再移除旧edition App，随后发布新App及其余组件，最后把旧CLI shim切到新edition。失败回滚先移除新App，再恢复旧App和原CLI，避免失败后双版并存或丢失旧App。

跨owner仍拒绝直接替换，需要原owner显式卸载后新owner安装；账号与配置资料不删除。测试覆盖install和replace两入口、6个阶段中断、旧App/CLI/record恢复、错误owner和旧签名拒绝。合计67项Python测试通过。README已移除默认拒绝replace、digest必填、verify需root及强制GUI认证回执的旧说法。

## G04 root bodyguard restoration (latest)
- Reused the root watchdog transaction for the fixed legacy bodyguard template. The embedded template is tested byte-for-byte against daemons/claude-bodyguard.sh after only the six-value proxy-mode substitution.
- Kept local.claude-bodyguard and local.tz-watchdog in legacy migration; both remain fixed root-owned system scripts. Root bodyguard retains the old cross-user Claude TERM range, 10-second interval and RunAtLoad. Rust user guardian remains.
- Backup/install/recovery covers scripts, plists and previous registration. Tests reject injection and user-owned existing scripts; fresh registration failure removes new files; replacement registration failure restores previous bytes and registration.
- Fresh verification: packaging Python 72 passed; diagnostics Rust 4 passed; py_compile and git diff --check succeeded. No real sudo, service operation, installation, app launch, account mutation or cross-user TERM executed. Actual root ownership and launchd behavior remain final-install acceptance, not claimed by fixture tests.

## Fresh HOME plan restoration (latest)
- Production plan now reads existing v2/legacy values first. When region values are absent, read-only curl probes Cloudflare country and ipinfo country/timezone, rejects CN/missing/conflicting observations, and maps locale. Proxy detection uses Clash Verge directory/first select group, Shadowrocket VPN listing, then auto.
- Original CLI snapshot remains separate from planned install literals. Backup race checks use only original bytes/metadata; commit writes exactly the planned five literals without redetecting network.
- Fresh verification: 78 isolated Python tests pass, including real production plan -> transact -> finalize fixture; first-run rejection produces no install/CLI writes; complete legacy/v2 configuration does not probe externally; proxy groups preserve dollars. py_compile and diff --check pass. All curl/scutil/launchctl calls mocked; no actual network or installation performed.
- Final first-run check: 79 tests pass. Production main now resolves plan and checks optional digest before lifecycle locks or app termination; rejected discovery has no installation side effects. Added direct main rejection test proving locks/stopped are not called.
- Administrator owner handoff remains design-only pending parent review; proposed reusing component backup plus old/new GUI registration snapshots and dual lifecycle locking. No owner handoff changes included in this patch.

## Administrator owner handoff (implemented after approval)
- Removed the previous-owner-uninstall gate for administrator install/replace. Plan pins previous owner and old App identity; transaction checks old record and component snapshot before staging writes. Uninstall still requires current owner.
- Split lifecycle locks from the one root lock. Old/new UID lifecycle locks are acquired in sorted order. Root journal captures original registration across both GUI domains plus the shared helper. Install retires old guardian, writes one owner record and registers only the new owner; recovery restores original registrations.
- Kept old-owner account/config/CLI content untouched. Only new-owner CLI participates in the existing backup/restore transaction. Same-edition and edition-changing owner handoff share the existing App rollback logic.
- Verification: 87 isolated packaging tests pass; py_compile and git diff --check pass. New coverage includes same/different edition success, failure phases, partial new-service registration failure, restoration of pre-existing new-owner CLI, immutable old-user content, owner-record race before writes, and sorted two-owner lifecycle locks with a single root lock.
- Boundaries: administrator-only mutations, one active owner/edition, existing user lifecycle directory/ownership checks retained. No real sudo, cross-user process termination, launchctl mutation or installation executed; actual macOS runtime remains final-install acceptance.

## Discovery compatibility corrections (latest)
- Fixed select-group section termination to stop only at the next top-level YAML mapping key; indentless list items remain in proxy-groups.
- Discovery curl alone inherits lowercase/uppercase http_proxy, https_proxy, all_proxy and no_proxy from the invoking environment. Root management commands retain the fixed minimal environment; other inherited variables remain excluded.
- Restored ordered geo fallback: ipinfo, ipwho.is, ip.sb. Each candidate must provide a valid timezone and country matching the Cloudflare observation; HTTP/curl failure, malformed JSON, missing fields and mismatch move to the next provider. Exhaustion fails before installation writes or app termination.
- Verification: 91 isolated packaging tests pass; py_compile and git diff --check pass. Added indentless YAML, exact subprocess environment boundary, simulated 429/bad JSON/missing fields/provider field shapes, fallback order and production-plan zero-write failure tests. No actual network request or privileged/system operation was run.
