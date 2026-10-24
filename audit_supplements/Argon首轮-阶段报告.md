一、 实际读取范围、未读缺口与无法运行的验证

受平台会话上下文窗口（Sliding Context Window）的物理机制限制，当前模型无法跨历史轮次重新访问第 1 至第 10 轮（01、02 初始附件及 G1-P01 至 G3-P12）的原始附件正文。按照阅读纪律，凡当前无法在上下文中直接回读正文并核对行号的文件，一律列为“未读”，绝不沿用历史摘要或凭哈希清单伪称已审。

1. 当前实际完整可读范围（第 11～17 轮：G3-P13 至 G4-P19，共 24 份分卷）

以下文件已在当前上下文中逐行核验正文、行段与具体实现事实：

打包与 macOS 本地安装器（完整）：

current/packaging/THIRD-PARTY-NOTICES.md (1-537, 538-651)：行 000030-000031 记录 cargo 508 项（缺项 148）、npm 84 项（缺项 34）；行 000023 记录 objc2 系列和 2 个 Rich Harris 包共 13 项上游专属版权通知缺口未关闭。   
TXT
+ 3

current/packaging/editions/{ipc-smoke,personal,public}.json (1-1)：personal.json 行 000001 赋予 personal-operations 能力（含 allow-publish-ad、allow-create-ad-signing-key 等 12 项广告运营权限）；public.json 行 000001 仅定义 productName 与 identifier。   
TXT
+ 2

current/packaging/macos/README.md (1-133)：行 000050 说明保留 local.tz-watchdog（3600s）与 local.claude-bodyguard（10s）两个 root 守护脚本，停用 SourceLocal 旧自动更新 Agent；行 000110 明确 SourceLocal 在进入 NativeUpdateCoordinator 前拒绝远程安装。   
TXT
+ 2

current/packaging/macos/build_pkg.py (1-205) & test_build_pkg.py (1-66)：行 000046-000065 生成的 PKG 脚本校验 /dev/console UID 及 component-installer-active 中的 kern.bootsessionuuid。   
TXT
+ 2

current/packaging/macos/launchd/local.claudeswitcher.{guardian,helper}.plist (1-10, 1-9)。   
TXT

current/packaging/macos/local_install.py (1-1617) & test_local_install.py (1-1253)：行 000281-000290 通过 ctypes 调用 renameatx_np (RENAME_EXCL = 4)；行 000440-000461 直接调用 libc.getgroups 规避 Python os.getgroups 在 macOS 上返回目录服务全量组导致的 setgroups 越界；行 000664-000687 实现 Cloudflare loc 校验（拒 CN）与 ipinfo.io 
→
→ ipwho.is 
→
→ api.ip.sb 三级回退；行 000812 内嵌 BODYGUARD_SCRIPT；行 000853-000879 定义 root_service_ownership 将继承的 gid 修正为 0:0；行 001462 finalize 默认 authenticated=False。   
TXT
+ 4

current/packaging/notices/{fetch_supplements.py,generate.py,macos_subset.py} (1-187, 1-241, 1-202)。   
TXT

current/scripts/{build-rust.sh,gen-sums.sh} (1-19, 1-14)：build-rust.sh 行 000019 通过 --features "$edition" 与 --no-default-features 构建对应版本。   
TXT
+ 1

工程根配置、文档与多平台旧版脚本（完整）：

current/.cargo/config.toml (1-2)、current/.gitignore (1-25)、current/AGENTS.md (1-112)、current/Cargo.lock (1-5426)、current/Cargo.toml (1-45)、current/LICENSE (1-21)、current/README.md (1-230)、current/SECURITY.md (1-27)、current/SHA256SUMS (1-8)、current/assets/icon-gen.swift (1-69)、current/rust-toolchain.toml (1-4)。   
TXT
+ 4

current/experiments/p0/**（含 desktop/、native/、protocol/ 下全部 26 个文件）。   
TXT
+ 4

current/linux/{README.md,claude-bodyguard.sh,claude-switch.sh,install.sh,stun-probe.py} 及 current/windows/{README.md,SwitcherUI.ps1,claude-bodyguard.ps1,claude-switch.ps1,install.ps1}。   
TXT
+ 3

current/tests/{test_proxy_diagnose.py,test_switch_safety.py} (1-182, 1-156)。   
TXT

全局账本、清单、图谱与终审证据集（完整）：

需求与工作流程账本.md (1-38)、当前项目代码图谱.md (1-80)、SOURCE_MANIFEST.json (1-3096)。   
TXT
+ 3

codegraph/{coverage.json,workspace-packages.json,file-relations.csv}。   
TXT
+ 1

evidence/final/**（16 个文件）、evidence/first-a-in-place/**（11 个文件）、evidence/restoration-tracker.json (1-876)、evidence/result.json (1-112)、evidence/root-group-fix/**（15 个文件）、evidence/source-before.json (1-64)。   
TXT
+ 4

2. 当前未读缺口（需补交后方可作全仓定论）

以下核心实现与原版对照材料位于第 1～10 轮历史附件中，当前会话无法直接读取其正文：

原版基线（original_HEAD/ 29 个文本文件）：尤其是 original_HEAD/core/claude-switch.sh、original_HEAD/app/main.swift、original_HEAD/installer/安装.command、original_HEAD/daemons/*.sh（需逐行核对旧版行为细节，不依赖 restoration-tracker.json 的二手转述）。

switcher-domain 源码（6 个文件）：crates/switcher-domain/src/{ads.rs,checks.rs,desktop.rs,lib.rs,system.rs} 及 bin/export-types.rs（原在 G1-P10）。

switcher-core 源码与测试（29 个文件）：crates/switcher-core/src/{accounts.rs,accounts/legacy.rs,ad_runtime.rs,ad_runtime/local.rs,ad_runtime/tests.rs,ads.rs,checks.rs,config.rs,diagnostics.rs,guardian.rs,guardian_repair.rs,install.rs,installation_health.rs,lib.rs,lifecycle.rs,macos.rs,migration.rs,network.rs,pinned_routes.rs,proxy_repair.rs,region_detection.rs,regions.rs,regions/legacy.rs,repair_cli.rs,signed.rs,storage.rs,time.rs,timezone_guard.rs,updates.rs,user_repairs.rs}、3 个 bin/*.rs 及 tests/{checks_cli.rs,e11_privacy.rs}（原在 G2-P01 至 G2-P15，仅 legacy.rs 在 G4-P17 的 change.diff 中读到 203 行局部上下文）。   
TXT

switcher-helper 源码（9 个文件）：crates/switcher-helper/src/{installation_health.rs,lib.rs,main.rs,native_authorization.rs,platform.rs,platform/dns_cache.rs,platform/timezone_guard.rs,policy.rs,root_operation.rs,xpc.rs}（原在 G3-P08 至 G3-P12）。

apps/desktop 前后端源码（20 个核心文件）：apps/desktop/src-tauri/src/{bridge.rs,commands.rs,main.rs,native_probe.rs,publisher_commands.rs,update_runtime.rs}、publisher/{backup.rs,local_store.rs,mod.rs,protocol.rs,security.rs,tests.rs}、src/{App.svelte,ui.ts,operations-date.ts}、src/components/*.svelte、src/pages/Operations.svelte（原在 G3-P01 至 G3-P08）。

历史与现行工程/PRD 文档：docs/ 下的 PRD、实施状态（如 docs/handoff/Rust-R4-实施状态.md）等（原在 G1 组）。

3. 无法运行的验证边界

本机真机特权与环境交互无法由模型运行：模型无法执行 sudo、无法操作真实 macOS launchd、无法挂载第二磁盘卷测试跨卷 ditto。

真实账号登录态与双账号切换未验证：根据 evidence/root-group-fix/native-runtime.json 行 000019-000020 与 000037，用户原账号已被封禁，本地无已登录的有效 Claude 会话基线，也没有真实账号 B；从已安装的 Personal 候选启动账号 A 仅观察到拉起 Claude 登录页及切换器自动退出，真实的已登录态保留及 A$\rightarrow
𝐵
B\rightarrow$A 往返切换尚未在真机验收。   
TXT

20261024-inplace 补丁候选尚未执行管理员安装：evidence/first-a-in-place/native-windows.json 行 000003 与 000058 证明最新原位建档补丁仅编译并观察了非安装态双版窗口与只读 IPC（12 通过 / 1 警告 / 1 时区阻断），尚未通过 sudo local_install.py 安装到系统目录。   
TXT

二、 对上一份 Pro 草稿推断的独立复核与纠偏

基于当前实际读到的 需求与工作流程账本.md、local_install.py、change.diff 及 evidence/**，上一份 Pro 回复存在多处未区分历史背景与当前代码事实的武断批评，特此纠偏：

关于“保留 update_runtime.rs 和 ad_runtime.rs 违背用户要求”的纠偏：

事实核对：需求与工作流程账本.md 行 000009-000010 明确记录用户最初交接即为“四个Rust包、12任务37验收；Personal手动更新、Public自动更新；暖白墨绿UI、三个广告位及完整广告运营”，后续指令为“先把项目系统本身做完。再考虑补广告”（行 000010）以及“除了本地版本因为安全问题阻止它自动更新以外，其他的逻辑都恢复到旧版本的逻辑”（行 000017）。   
TXT

结论：“广告后置”不等于“取消广告或删除已有广告代码”；“本地版本（SourceLocal）不自动更新”也不等于要删除 Public 版的签名更新架构。在实测证据 evidence/root-group-fix/source-local-update-native.json 行 000003-000005 中，已安装 CLI 执行 update --auto 明确返回退出码 2 及 "source_local_rebuild_required：请从源码重新构建并由本地安装器更新"，证明 SourceLocal 禁止自动更新在入口行为上已严格落实。上一份 Pro 草稿仅凭模块存在就要求删除全部更新与广告模块，属于误判。   
TXT
+ 1

关于“local_install.py 写入 gid 80 (admin) 是典型提权漏洞且应废弃 Python 安装器”的纠偏：

事实核对：

文件权限与可写性：在修复前的诊断记录 evidence/root-group-fix/actual-before.json 行 000053-000067 中，tz-watchdog.sh 与 claude-bodyguard.sh 的属性为 uid: 0, gid: 80, mode: "0o755"。八进制 0755 意味着组权限（Group）和其他人权限（Other）均为 r-x（只读+可执行，无 w 写权限），且 local_install.py 行 000072 的 tree() 严格拒绝任何带 0o7022（组或其他可写）的文件。因此，即便修复前 gid 继承了父目录的 80 (admin)，admin 组非 root 成员也没有写权限，上一份草稿称其为“普通管理员进程可篡改守护脚本的提权漏洞”在 UNIX 权限位事实上不成立（除非叠加目录替换，而父目录 /Library/Application Support/claude-switcher 自身为 uid: 0, gid: 80, mode: 0o755，同样无组写权限）。   
TXT
+ 2

历史缺陷与当前冻结状态：该属组不一致导致的问题是 local_install.py 行 000915 的严格自检（要求 st_gid == 0）报错退出（local.tz-watchdog installation differs）。当前冻结版 local_install.py 行 000853-000905 已通过 root_service_ownership() 在无符号链接跟随的描述符上执行 os.fchown(fd, 0, 0) 修复，且 evidence/root-group-fix/native-committed.json 行 000030-000053 证明真实安装提交后 4 个系统守护文件均已为 uid: 0, gid: 0。   
TXT
+ 3

Python 安装器的必要性：local_install.py 承担了 App/Helper/Guardian 的 CDHash 提取与 Hardened Runtime 校验（行 000081-000105）、renameatx_np(RENAME_EXCL) 防符号链接竞态替换（行 000281-000335）、跨 UID 生命周期锁与失败事务回滚（行 001067-001113, 001326-001373）。将其废弃退回裸 bash 脚本反而会丢失事务回滚与防竞态保护。   
TXT
+ 2

关于“全部测试均为 Mock、无任何原生证据”的纠偏：

虽然代理与跨卷切换确有 NOT_RUN 项，但 evidence/root-group-fix/native-committed.json（真实 root 安装提交与 launchd helper/guardian 进程存活验证）、native-cli-after-commit.json（真实 shim CLI 跑完 14 项检查）、native-runtime.json（真实拉起 Personal 窗口并启动 Claude 进程） 及 evidence/first-a-in-place/native-windows.json（双版本原生窗口与 IPC 实测） 均为真实的本机原生执行证据，不能一概否定。   
TXT
+ 3

三、 阶段性审核报告（仅基于当前已读的 24 份分卷）

声明：以下 A～E 结论仅在当前已完整读取的打包脚本、全局清单、测试与证据文件范围内成立；针对 crates/switcher-* 与 apps/desktop 内部逻辑的最终结论，必须待补交上文“未读缺口”后再行合入。

A. 目标与范围偏离

双重守护进程并存（Legacy Root Bodyguard 与新 User Guardian 同时运行）

证据：current/packaging/macos/README.md 行 000050 与 current/packaging/macos/local_install.py 行 000812-000823、001318-001322。安装器在 bootstrap 新版用户态 local.claudeswitcher.guardian 的同时，还物化并启动了旧版每 10 秒轮询一次的 root 守护 local.claude-bodyguard（执行 pkill -f '/Applications/Claude.app/Contents/MacOS/Claude'）。   
TXT
+ 3

影响：两套守护进程同时以 10 秒周期监控代理与 TUN 并杀进程。虽然是为了恢复 G04（跨登录用户终止 Claude 的 root 范围），但造成了后台双倍轮询开销与职责重叠。   
TXT
+ 1

最小修复与验证：待补读 crates/switcher-core/src/guardian.rs 后，核实用户态 Guardian 与 root 保镖的触发边界是否冲突；若仅需单机单用户使用，应在文档中明确说明双守护并存的原因，或在配置中允许收敛。

第三方许可证声明材料存在未关闭缺口（若立刻开源发布）

证据：current/packaging/THIRD-PARTY-NOTICES.md 行 000023-000031 及 current/packaging/notices/macos_subset.py 行 000173-000175。在有效构建图中，仍有 11 个 objc2-* crate 和 2 个 npm 包（is-reference、locate-character）仅有标准许可证模板而无上游专属版权通知。   
TXT
+ 3

影响：用户计划将项目开源到 GitHub。当前文档虽诚实标注了“条款已归档不代表这 13 项通知缺口关闭”，但若直接分发二进制包，仍需人工核对上游仓库历史以补齐署名。   
TXT
+ 1

最小修复与验证：仅交付源码时保持当前 THIRD-PARTY-NOTICES.md 的诚实声明即可；若后续发布预编译 Release 包，需按 MISSING.md 补齐对应上游提交的版权头说明。

B. 真实缺陷和严重度

[中危 / 状态不同步] 已安装的系统运行环境与最新源码补丁（20261024-inplace）尚未对齐

证据：evidence/result.json 行 000042、000078、000095-000109，以及 evidence/first-a-in-place/change.diff 行 000028-000034。2026-10-23 安装到 /Applications 的版本在首次收编账号 A 时，执行了 copy(&store.active, &incoming) 并做目录交换，导致活动目录 ~/Library/Application Support/Claude 的 inode 从 15628535 变为 61761011。虽然 crates/switcher-core/src/accounts/legacy.rs 已通过引入 in_place = target == current 修复了该缺陷（行 000031），但该补丁编译出的新包尚未执行管理员安装。   
TXT
+ 4

影响：当前机器 /Applications/Claude 切换器 Personal.app 仍是旧二进制，若在清空状态后触发首次 A 建档，仍会替换活动目录 inode。

最小修复与验证：全仓源码复审通过后，使用最新构建的 20261024-inplace 产物执行一次 sudo python3 packaging/macos/local_install.py install --edition personal 完成覆盖安装，并运行 local_install.py verify 验证。

[低危 / 环境配置不匹配] 当前测试机系统时区（Asia/Shanghai）与已保存目标（America/Los_Angeles）不一致，阻断启动

证据：evidence/first-a-in-place/native-windows.json 行 000016-000018、000048-000050 及 evidence/root-group-fix/native-cli-after-commit.json 行 000023-000024。14 项安检中“时区”一项为 fail（block: 1）。   
TXT
+ 3

影响：由于 Fail-Closed 机制生效，当前在测试机上直接点击启动会被时区检查合法拦截。local.tz-watchdog 的 StartInterval 为 3600 秒，在未满 1 小时或未触发 RunAtLoad 时不会立即重置系统时区。   
TXT
+ 1

最小修复与验证：属于预期的环境安检拦截行为（非代码 Bug）。在真机验收启动前，可执行一次一键修复（fix / set-region）或等待/触发 tz-watchdog 同步时区。

[低危 / 构建警告] ts-rs 无法解析 #[serde(deny_unknown_fields)] 产生大量编译期警告

证据：evidence/final/build-personal-final.log 行 000001-000022、000049-000084（每次编译重复输出十余次 warning: failed to parse serde attribute: deny_unknown_fields）。   
TXT

影响：不影响运行期 serde 的严格反序列化校验，但污染构建与测试日志，容易掩盖其他有价值的编译器警告。

最小修复与验证：待补读 crates/switcher-domain/src/*.rs 后，检查是否可通过升级 ts-rs 或调整属性宏顺序消除警告。

C. 过度设计与删除/合并建议

仓库内保留了完整的 experiments/p0/ 早期技术验证工程（26 个文件）

证据：SOURCE_MANIFEST.json 行 001400-001636 及 G4-P05 至 G4-P10 正文。其中包含独立的 experiments/p0/desktop/package-lock.json（1442 行） 与 experiments/p0/desktop/src-tauri/Cargo.lock（4879 行）。   
TXT
+ 4

影响：虽然在根 Cargo.toml 行 000003 中已通过 exclude 排除，但占据了约 6,500+ 行锁文件与探针代码，增加仓库维护噪音。   
TXT

建议：若无需保留早期 P0 历史探针作合规备查，可在最终清理阶段归档或移除 experiments/p0/；若需保留作为工程阶段证据，则维持现状（当前无运行时耦合）。

switcher-core 中存在新旧两套账号与地区实现（accounts.rs vs accounts/legacy.rs、regions.rs vs regions/legacy.rs）

证据：SOURCE_MANIFEST.json 行 000987-000999（accounts.rs 2813 行 + accounts/legacy.rs 1013 行）及行 001162-001174（regions.rs 2816 行 + regions/legacy.rs 544 行）；codegraph/file-relations.csv 行 000219、000234、000480。   
TXT
+ 1

影响：两套状态机与日志格式并存，显著增加了代码体积与心智负担。但根据 需求与工作流程账本.md 行 000036 提示，“无调用不等于可立即删除；检查恢复旧journal、CLI入口及历史状态兼容”。   
TXT

建议：当前严禁盲目删除。必须等补交 accounts.rs、accounts/legacy.rs、migration.rs、regions.rs 与 bridge.rs 正文后，逐行核查生产调用链究竟走哪条分支、旧 v2 journal 恢复是否仍依赖非 legacy 模块，再给出精确的裁剪或合并方案。

D. 测试与完成证据的可信边界

根据 evidence/result.json 与 evidence/restoration-tracker.json 交叉核对：   
TXT
+ 1

已充分验证（可信）：

local_install.py 的 97 项隔离故障注入与权限恢复测试（含 gid 80 -> 0 修复、跨 owner 交接、同 owner Personal/Public 换版、CLI shim 单引号转义、Cloudflare+3级 geo 回退）。   
TXT
+ 1

20261023 候选包的真实管理员安装、launchd 服务（helper、guardian、tz-watchdog、claude-bodyguard）注册、root 守护脚本 0:0 权限落盘。   
TXT

原位建档补丁（first-a-in-place）在隔离环境下的 inode 保留、软硬链接与 ACL 保持、6 个中断点的幂等恢复及旧 journal 兼容测试。   
TXT

双版本（Personal / Public）Release 构建、Ad-hoc 签名严格校验、原生窗口渲染及只读 IPC 14 项安检返回。   
TXT
+ 2

尚未验证（不可夸大为已通过）：

真实代理客户端联动未在真机执行：P01～P09（Shadowrocket / ClashX / Surge / Clash Verge 的真实自动拉起、verge.yaml 修改重启、Merge.yaml 置顶规则生效）仅通过模拟命令测试（not_run_final_candidate）。   
TXT
+ 1

真实账号登录态下的 A$\leftrightarrow$B 切换未验证：受限于无有效 Claude 登录账号，未在真机验证双账号真实 OAuth 登录态的封存与切换。   
TXT

20261024-inplace 补丁包尚未执行真实管理员覆盖安装。   
TXT
+ 1

E. 下一步最小行动顺序（先补齐缺口，再定最终整改）

为确保全仓审核真正做到逐文件、逐行号有据可查，请不要现在修改任何代码，按以下优先级分批补交当前会话无法访问的核心源码文件：

第一优先级（核心业务与恢复链路，最关键）：

原版对照：original_HEAD/core/claude-switch.sh、original_HEAD/app/main.swift、original_HEAD/installer/安装.command。

当前核心：current/crates/switcher-core/src/{lifecycle.rs,checks.rs,macos.rs,network.rs,config.rs,proxy_repair.rs,pinned_routes.rs}。

第二优先级（账号、修复与守护双轨逻辑，用于判定冗余与可达性）：

current/crates/switcher-core/src/{accounts.rs,accounts/legacy.rs,migration.rs,regions.rs,regions/legacy.rs,user_repairs.rs,guardian.rs,guardian_repair.rs,timezone_guard.rs,repair_cli.rs} 及 bin/{claude-switcher.rs,switcher-guardian.rs}。

第三优先级（桌面端桥接、特权 Helper 与领域契约）：

current/apps/desktop/src-tauri/src/{main.rs,bridge.rs,commands.rs,update_runtime.rs,publisher_commands.rs} 及 src/App.svelte。

current/crates/switcher-helper/src/{main.rs,xpc.rs,policy.rs,platform.rs,root_operation.rs,installation_health.rs}。

current/crates/switcher-domain/src/{lib.rs,system.rs,checks.rs,desktop.rs,ads.rs}。

第四优先级（后置的广告/更新内部实现与文档）：

current/crates/switcher-core/src/{ads.rs,ad_runtime.rs,updates.rs,install.rs,signed.rs,storage.rs,time.rs}、apps/desktop/src-tauri/src/publisher/* 及 docs/ 下的 PRD 与交接状态文档。

请分批提供上述缺失文件的文本分卷，我将在逐份核对具体行号与实现逻辑后，为您合入并输出最终的完整五部分总报告。
