# GPT Pro 审计独立核实：运行与兼容分组

日期：2026-10-25。交接编号：CS-GPTAUDIT-20261025-A1。
工作树：`/Users/gua/CodexHome/worktrees/36b7/claude-switcher`。
核实 HEAD：`98c42fc6825ab90fe018032ae4243883e988d856`。
范围：F04、F05、F06、F08、F09、F10、F13、F18。

## 证据边界

已核对实际 cwd、Git 根与 HEAD。CodeGraph status 为 complete，153 个索引文件，pendingChanges 为零。先通过 CodeGraph 定位；仅对未返回的具体区间及非索引 shell/文档作补读。审计报告仅作为待核实主张。此记录不将报告的置信度当运行证据。

本轮只作静态核实。没有产品测试、构建、安装、联网、系统设置修改、真实账号访问或业务源码修改。因此下文“采纳”表示接受整改问题及计划，不表示已运行重现，更不表示已修复。旧实现对照使用本提交保留的 `core/claude-switch.sh` 与 `app/main.swift`；不把其当前行号冒充报告 original_HEAD 的历史行号。

| 条目 | 判断 | 当前证据 | 置信度 |
|---|---|---|---|
| F04 | 采纳 | 相同文件阻止运行重载的分支已静态确认 | 高（分支）；运行待验证 |
| F05 | 采纳 | 两轮 40 次等待串联，第一轮错误被丢弃 | 高（调用链）；真实耗时待验证 |
| F06 | 采纳 | 旧 shim 无格式适配，状态/成功文本/部分失败码不同 | 高（协议）；外部脚本损坏未观察 |
| F08 | 采纳 | dig 的 3 秒预算被外层 2 秒预算截断 | 高（阈值）；延时答复待验证 |
| F09 | 采纳 | GUI 两步修复且成功后未自动全重检 | 高（前端调用链）；UI 实跑待验证 |
| F10 | 采纳 | 注释/尾空格不匹配，进入追加同名键分支 | 高（字符串逻辑）；代理解析待验证 |
| F13 | 部分采纳 | 重复校时及重复停机执行路径存在 | 高（存在性）；停机执行者取舍待定 |
| F18 | 部分采纳 | 准备 API 合同冲突和旧浏览器脚本前提过时 | 高（局部矛盾）；广泛删减待完整调用核对 |

## F04：文件正确但运行未加载

**直接证据**：`crates/switcher-core/src/user_repairs.rs:132` 在路由已通过时提前返回；`:145` 在运行配置含目标规则时提前返回；两者均不通过才生成 Merge 提案。`:155–157` 因提案与文件相同返回 false。`crates/switcher-core/src/regions/legacy.rs:148–155` 仅在返回 true 时执行 stop、start、verify。这把“文件需要写入”同时用作“运行需要重载”。

**触发条件（未执行）**：路由检查失败，运行配置无目标规则，但 Merge.yaml 已等于生成目标。连续调用修复会连续跳过重载。旧保留实现 `core/claude-switch.sh:735–748` 只要 merge_ok 与 route_ok 都失败，就写入并重载，没有文件字节相同门槛。

**测试对照**：`user_repairs.rs:2119–2144` 验证初次创建、路由已通过不写、运行配置已有规则不写；未覆盖“Merge 正确但运行配置未加载”的调用方组合。`pinned_routes.rs:275–276` 证明提案字节幂等，是正确文件语义，不能据此推出免重载。

**最小未来变更**：把重载条件与文件 changed 分开；保留已成功路由或运行配置的旧 OR 语义。不要为修此问题每次无条件重启所有代理。

**验证**：临时 HOME 与假 Runtime，先生成目标 Merge，再保持运行配置缺规则且路由失败；第二次修复应不写文件但重载一次。重载后重新核对规则/路由。已成功状态应不重载。

## F05：代理失败双重等待

**直接证据**：`regions/legacy.rs:98–99` 调用 execute_app_open 后丢弃 Result。`proxy_repair.rs:184–193` 在必要时自动打开 App，随后执行 40 次 ready/pause；`:217–218` 每次 pause 为 1 秒。返回失败后，`regions/legacy.rs:117–127` 再执行 40 次检查与 1 秒 sleep。

**限制**：持续“不就绪且检查正常返回 false”的情况至少包含 80 次一秒 sleep；不是所有失败都固定 80 秒。命令成本可增加耗时；底层早返回的错误可缩短第一轮。Clash Verge 还包含 TUN 修复，不能简单按其他客户端路径删掉步骤。

**旧行为及测试**：`core/claude-switch.sh:668–682` 对 Shadowrocket/ClashX/Surge 仅一轮 40 次。`proxy_repair.rs:293–309` 只锁定底层 40 次暂停，未覆盖外层再次等待。

**最小未来变更**：只让一个组合入口拥有等待预算；打开 App、Clash Verge TUN 恢复、就绪等待分别返回明确结果。保留 Shadowrocket 与 ClashX 自动打开。不要直接把吞错改成 `?` 后破坏 Verge 的后续 TUN 修复。

**验证**：假时钟、假 opener 与假就绪检查，从真实组合入口测一直未就绪、39 次后就绪、打开失败、Verge TUN 为 false。记录总预算与打开次数，不用真实应用或真实等待。

## F06：旧 CLI 输出协议

**直接证据**：`packaging/macos/local_install.py:558–569` 在旧 `.local/bin/claude-switch.sh` 写出直接 exec Rust CLI 的 shim，无兼容参数。`crates/switcher-core/src/bin/claude-switcher.rs:84–101` 的 status 输出 JSON；`:195–200` 的 check 成功只输出 OK；`:143–154` 的 switch 未完成返回 2。

**旧行为**：`core/claude-switch.sh:905–909` 输出 `current=none hasA=0 hasB=0` 等键值行；`:369` 的 check 成功包含出口与可选警告；`:935` 的启动预检失败返回 1。这里不写入任何真实出口值。

**额外确认**：`accounts.rs:281–291` 在根目录不存在时报告 current=A；`:398–405` 缺 marker/state 也默认 A。`tests/checks_cli.rs:154–160` 明确锁定空 HOME 的 JSON current=A；它验证新合同，无法证明旧合同兼容。

**最小未来变更**：给旧 shim 显式兼容格式，结构化输出仍明确可选。将“没有选择记录”与内部默认目标 A 分开；不要为了 status 改坏首次切换逻辑。按旧命令逐项对照文本、stdout/stderr、退出码。check 的出口字段需要在兼容入口按既有隐私边界处理，不复制进日志或审计文档。

**验证**：隔离 HOME 的无目录、A、B、缺 marker、有存档、预检失败、同账号、切换失败矩阵；对旧输出与兼容输出作字节/退出码对照。使用合成网络值。没有观察到用户外部脚本实际损坏。

## F08：DNS 超时矛盾

**直接证据**：`crates/switcher-core/src/macos.rs:287` 调用 dig `+time=3 +tries=1`；通用执行器 `:164–169` 两秒超时后 kill/wait。`:140` 另保留 kill_on_drop。旧 `core/claude-switch.sh:223`、`:327` 的 dig 为三秒且没有该两秒外层。

**推断条件**：有效首条 DNS 答复在 2–3 秒间返回且无其他失败，Rust 调用已超时。尚未运行延时答复重现。`checks.rs:772–786` 使用命令结果夹具，不会穿过实际子进程超时分支。

**最小未来变更**：给 DNS 专用执行预算，覆盖命令三秒及收尾；其他命令仍保留现有预算、输出上限与取消回收。

**验证**：无需网络的受控子进程夹具，分别模拟 1.9 秒、2.5 秒、超新预算输出及取消。确认 2.5 秒合法答复可读，超时和取消均回收子进程，不更改 DNS 通过判据。

## F09：GUI 修复交互与重检

**直接证据**：`apps/desktop/src/components/RegionSettings.svelte:21–33` 只产生 preview；`:35–40` 单独 execute；`:73` 和 `:78` 是两次点击。成功后仅 `load()` 查询 region_view。`apps/desktop/src/App.svelte:61` 的 announce 只保存 outcome；`:246` 用 announce 接收修复结果，没有 runChecks。`apps/desktop/src-tauri/src/bridge.rs:417–420` 清后端报告并要求重新检查。

**旧行为**：`app/main.swift:139–146` 一次 fix 调用后 refresh。当前 `App.svelte:199` 每 30 秒同步 snapshot，故旧本地分数可能短暂保留；未实跑 UI 测时。不能从此推出启动门禁被绕过。

**最小未来变更**：普通环境修复一次点击完成预备与执行；完成后立即清理旧报告并跑全检查。地区切换有实际选择，不必因普通 fix 的需求一并删除地区预览。必要系统授权与未完成事务恢复保持。

**验证**：前端 IPC 假实现记录调用顺序：一次普通修复点击只能发起一次执行，随后触发 check_environment；取消/失败不伪报完成；重新进入页面不使用过期分数。保持现有 UI 视觉与按钮反馈。

## F10：合法 YAML 注释造成同名键

**直接证据**：`crates/switcher-core/src/pinned_routes.rs:215–229` 只去掉 CR/LF 后与 `prepend-rules:` 精确比较。输入 `prepend-rules: # user rules` 或 `prepend-rules: ` 不匹配；inserted 仍 false，函数在 clean 前再加一个同名键。`core/claude-switch.sh:602–609` 用键前缀识别，因此这两种形式不会进入旧版新增键分支。

**限制**：重复键输出由静态字符串路径可推出；具体 Clash YAML 解析器如何拒绝或取值未运行验证。文档分隔头、inline 数组等不自动归为此次新增回归。

**测试对照**：`pinned_routes.rs:259–296` 覆盖标准键、用户规则、嵌套键、标记与部分复杂输入；已读测试未覆盖行尾注释/尾空格这一分支。

**最小未来变更**：正确识别顶层映射键及合法行尾注释/空白；仅编辑管理规则块，保留用户规则与其余原文。避免将任意 `prepend-rules: ...` 都视为块序列而误改 inline 值。

**验证**：输入标准键、注释、空格、CRLF、嵌套同名键、inline 空/非空列表、已有多个同名键、文档头。用解析结果和幂等性共同验证；输出只能有一个有效顶层键且用户规则保留。

## F13：重复保镖与用户校时

**直接证据**：`packaging/macos/local_install.py:772–789` 生成 root 每小时校时服务；`:815–822` 生成 root 每十秒保镖；`:1318–1322` 安装二者并 bootstrap 用户服务。`guardian.rs:534` 启动 timezone_guard；`:541–542` 接入同类代理/TUN 检查与停机；`:737–745` 同时承载安装健康 tick 与 Public 更新调度。`timezone_guard.rs:75–84` 从用户进程直接执行 systemsetup，未使用 helper 授权调用。

**判断**：采纳移除重复用户校时的方向。采纳梳理重复停机职责的需求；尚不采纳直接删除整套用户 guardian。用户 guardian 还承载事件响应、健康与受信 Public 调度。root 与用户执行者的权限/事件/配置来源不同，不能仅凭同责即认定完全等价。没有实际改时区，未观察现场失败或误杀。

**测试限制**：`timezone_guard.rs:158–169` 使用 reconcile_with 的假 call；`:182–199` 验证调度。这不证明普通用户执行 systemsetup 成功。

**最小未来变更**：先只移除用户直接校时路径，保留 root 小时校时；下一小步列出 stop 的唯一责任与被保留的事件响应。若选择 root 十秒路径，明确事件响应变化；若选择 Rust 停机，必须先证明旧服务不再必需且卸载/恢复兼容完整。不要顺带删 Public 支持或本地健康能力。

**验证**：合成调度/命令审计证明用户不再执行 systemsetup；静态安装清单与代理失联夹具证明只剩预定停机执行者；现存旧服务恢复场景仍可用。真机服务行为后续统一候选安装再验。

## F18：过时准备接口及宽泛删减

**直接证据**：`migration.rs:45–54` 固定返回 VerifiedPreserved；`:155–160` readiness=true。`bridge.rs:585–595` 只接受 HandlingPlanAvailable，否则加入 keychain_dependencies_unresolved；`:608` 因此投影 ready=false。`commands.rs:43–55` 仍提供准备/备份命令；`main.rs:129–130` 与 `:155–156` 在两版注册。当前前端 src 检索没有 migration_preparation/create_migration_backup 调用，账号页使用直接 account_action（`App.svelte:240–242`）。

**测试矛盾**：`apps/desktop/src-tauri/tests/e03_core_preparation.rs:217–225` 仍要求 VerifiedPreserved 被阻断。`apps/desktop/scripts/activity-refresh-regression.py:28–30` 在 check 时添加记录，`:42–44` 却期待开页后的活动为空；`App.svelte:193–194` 已开窗自动检查。`apps/desktop/package.json` test 仅运行 tests/*.test.mjs 与 core-preparation-regression.mjs，不包含此 Python 浏览器脚本。此处未运行脚本，属于静态前提冲突。

**限制与不同意扩张**：`main.rs:187–193` 的 updater 注册受 compiled_release_pins 条件约束，`update_runtime.rs:54–55` 当前为 None；不能说本地构建正自动更新。报告关于 helper timezone action、严格协调器及无消费代次的全面删除，本分工未完成其全部生产/恢复调用和协议兼容核对，保持待验证；不批准以 F18 为由整文件删除。

**最小未来变更**：分开处理准备接口与过时脚本。先确认外部 CLI 合同，再删除当前 UI 无消费的准备写入口、错误投影及对应权限/DTO/测试；更新活动脚本为开窗自动检查的现行行为。备份 CLI 已明确返回不支持（`migration.rs:267–270`），移除时需留清楚错误或兼容策略。

**保留边界**：`migration.rs:230–265` 仍读取并恢复 in-place-journal，必须保留。账号/修复/安装的旧 journal 解码和恢复不可随准备入口删除。updater/helper/严格协调器另列生产调用者和恢复入口表后再裁，不作为首批核心整改前置。

## 建议最小批次

1. 代理与配置：F04、F05、F08、F10 各自先加针对触发条件的隔离复现，再逐项修复。保持自动打开与退出顺序。
2. 旧入口：F06 的兼容矩阵与 F09 的普通一键修复/自动重检。保持地区选择及安全检查。
3. 收敛维护：F13 先删重复用户校时；F18 先处理无消费准备入口和过时脚本。停机职责及宽泛清理须先补调用证据。

```text
隔离复现 -> 单项最小修复 -> 针对验证
                              |
                              v
                    统一候选与后续安装验收
```

结束状态：本分工静态核实完成。以上未来变更均未执行。首批应先补 F04/F05 真实组合调用夹具，避免继续只证明单层假实现。
