# 安装与 helper 审核记录

交接编号：CS-GPTAUDIT-20261025-A1。核实日期：2026-10-25。

核实工作树：`/Users/gua/CodexHome/worktrees/36b7/claude-switcher`。
核实 HEAD：`98c42fc6825ab90fe018032ae4243883e988d856`。
范围：报告 F02、F03、F07、F11、F12。

本轮没有修改业务源码，没有构建、安装、产品测试、外网请求、系统设置写入或真实账号访问。CodeGraph 查询在源码检索前完成。下述“直接证据”包括当前源码和受限合成探针；不代表真机产品复现。

## 结论

| 条目 | 建议 | 置信度 | 当前证明范围 |
|---|---|---|---|
| F02 | 采纳 | 高 | 卸载管理清单遗漏两个 root 服务及 CLI；缩小后的完成条件可报告成功 |
| F03 | 采纳 | 高 | 安装器生成的 plist 语义等价但字节不符；合法时区更新改变了验证要求的引号 |
| F07 | 部分采纳 | 高（已确认部分） | 必要选择被静默默认；安装交易没有旧安装的完整环境收尾。未把首次 GUI 的完整行为作为本分工已验证事实 |
| F11 | 部分采纳 | 高（固定名冲突机制） | 三个固定临时文件可能阻塞各自后续写入。不能扩大为所有恢复必然失败 |
| F12 | 采纳 | 高（控制流） | 非本地安装 pending 条件下，启动自动恢复出错会在 XPC 启动前退出 |

## F02：卸载对象不对称

**直接证据**

- `packaging/macos/local_install.py:140`：uninstall 不采集 `cli_entry`，因此不采集时区和代理模式。
- `packaging/macos/local_install.py:147`：destinations 仅包含 App/helper/guardian/新 daemon/新 agent/install record。
- `packaging/macos/local_install.py:618`、`:825`、`:831`、`:890`：空 CLI entry 和空目标令 CLI/两个 root 服务的独立安装处理跳过。
- `packaging/macos/local_install.py:1002`：legacy_migrate 对 `local.tz-watchdog` 和 `local.claude-bodyguard` 明确 continue。
- `packaging/macos/local_install.py:1376`、`:1469`：完成判断只枚举新 helper/guardian 与 destinations。
- `packaging/macos/local_install.py:567`：留下的 shim 指向被卸载的 App 内 Rust CLI。
- 旧实现 `installer/卸载.command:7`、`:10`、`:15` 会停止旧 root 服务，移除 plist 与 CLI。旧实现的宽泛删除不应照搬。

**受限合成探针**

从当前 Python AST 仅提取 `legacy_migrate` 与 `finalize`；所有 I/O 替换为内存 stub。输入仅包含上述两个旧 root 服务时，迁移操作调用列表为 `[]`。在 destinations 和新服务集合为空时，finalize 返回 `status=uninstalled`，证据字段为 `components_absent=true, services_absent=true`。这确认完成条件遗漏。未运行真机卸载，也没有观察服务实际继续 TERM Claude。

**现有测试限制**

`packaging/macos/test_local_install.py:34` 默认把 legacy_snapshot 设为 `[]`；`:46` 的手工 plan 仅列六个主组件。`:116` 与 `:238` 的卸载测试沿用此模型，未证明新增 root 服务与 CLI 被卸载。测试源码已审阅，本轮未执行。

**最小整改**

修改 `packaging/macos/local_install.py` 和对应隔离测试。将本安装拥有的 CLI、两个 root 脚本及 plist 纳入卸载预览、备份、停止、删除、回滚、完成验证。删除前验证归属、内容及文件类型。保留账号、配置、历史备份与修复 journal。

**最小未来验证**

真实 plan 生成 → 隔离 install → uninstall → finalize；断言 CLI 与全部四类服务组件消失。逐个卸载中断点恢复原字节、权限与服务注册状态。增加外部替换/未知归属拒绝分支。

## F03：相同系统对象使用不同字节合同

**直接证据**

- `packaging/macos/local_install.py:776` 生成 `TARGET="zone"`；`:785`、`:818` 通过 plistlib 序列化两个 plist。
- `crates/switcher-core/src/guardian_repair.rs:349` 至 `:351` 对 plist 比较完整字节；`:383` 至 `:394` 的比较来源是 `daemons/*.plist`。
- `crates/switcher-core/src/guardian_repair.rs:397`：已注册服务直接通过；只有需要补注册时才触发该模板验证。不能描述为每次正常 fix 均失败。
- `crates/switcher-core/src/guardian_repair.rs:478` 至 `:493`：合法目标时区变化时，sed 写出无引号 `TARGET=zone`。
- `packaging/macos/local_install.py:908` 至 `:919` 按安装器当前 payload 全字节比较。`:1555` 的公开 verify 会调用此判断。

**受限合成探针**

只提取当前 payload 函数，在内存生成产物。两个 plist 与 daemons 模板的结果均是 `bytes_equal=false, parsed_values_equal=true`。将 Tokyo payload 按当前 sed 规则变为 New York 后，产物为 `TARGET=America/New_York`；当前验证期待 `TARGET="America/New_York"`，完整字节不相等。探针没有执行 shell、launchctl 或 Rust 验证函数。

**现有测试限制**

`crates/switcher-core/src/guardian_repair.rs:234` 检查自己的 TEMPLATE 和任意坏数据；`:432` 起主要注入布尔组件状态。`packaging/macos/test_local_install.py:1040` 解析安装器 plist 并检查引号，未将产物送给真实 Rust 比较入口。因此现有测试不能覆盖两端合同。

**最小整改**

统一 `packaging/macos/local_install.py`、`crates/switcher-core/src/guardian_repair.rs` 与固定 daemon 模板的编码合同。选一种规范生成方式，并兼容已经合法安装的等价产物。若采用 plist 语义比较，严格限定键、参数、路径和类型，拒绝额外执行键。时区更新使用同一合法格式。保留 root 所有权、权限、链接与固定路径检查。

**最小未来验证**

安装器真实产物 → Rust 修复验证 → 未注册服务 bootstrap 计划；合法 set-region → installer verify。恶意额外 plist 键、错误可执行路径、可写文件、链接仍拒绝。

## F07：选择和安装收尾

**直接证据**

- `packaging/macos/local_install.py:690` 至 `:711`：找到第一个 select 立即 break；无 Verge/Shadowrocket 时返回 auto。
- `packaging/macos/local_install.py:714` 至 `:746`：优先保留已有设置，仅缺值时探测。这一点应保留，不能改为每次安装都询问。
- `installer/安装.command:53` 至 `:64`：旧版未知客户端提供选择。`:112` 至 `:139`：零个/多个组要求确认，单组自动采用。
- `packaging/macos/local_install.py:1317` 至 `:1323`：交易尾部安装 shim、两个 root 服务并启动新服务；`:1482` 至 `:1485` 的默认 finalize 只验组件及进程，明确记录 `authenticated_health=not_run`。
- `packaging/macos/local_install.py:1598`、`:1609` 没有传 `authenticated=True`；即使 launch=True，也会先走上述默认分支。
- 旧 `installer/安装.command:265` 写置顶规则；`:318` 至 `:321` 按需设置时区/IPv6并刷新 DNS；`:357` 至 `:369` 重启 Verge并 checkdetail。

**受限合成探针**

在自动清理的临时 HOME 内放两个 select 组 First/Second。执行 AST 提取的实际 detect_proxy，返回 First。对无客户端目录与空 VPN 输出的合成输入，返回 auto/GLOBAL。只读合成文件，无真实 VPN 或网络调用。

**边界修正**

“组件安装未包含完整环境配置”有直接证据；不能扩写为“安装完全不改时区”，因为新注册的 RunAtLoad 时区 watchdog 会尝试校时。首次 GUI 的完整自动执行路径本分工未继续验证，因此不把报告的该子结论当已重现。无证据证明现有配置已满足时会产生实际用户故障。

**最小整改**

安装脚本仅在缺少明确配置且候选不唯一时提供选择。收尾复用既有修复与检查入口，输出组件完成与环境未完成的区别；保留失败条目。先明确如何复用当前修复入口，避免安装器重新实现所有规则。预计变更 `packaging/macos/local_install.py`、当前本地安装入口及对应测试；是否需要 Rust 新入口由后续调用链核实决定。

**最小未来验证**

零/一/多 select、未知客户端、已有明确配置矩阵；真实安装尾部调用链的合成 runner 记录路由/IPv6/DNS/Verge reload/checkdetail，校验动作只执行一次及失败不被报告为全完成。

## F11：固定临时名残留

**直接证据**

- `crates/switcher-helper/src/platform.rs:370` 至 `:383`：`.component-committed.next`，create_new 后写入并 rename。
- 同文件 `:611` 至 `:626`：`.repair-journal.next`；`:934` 至 `:950`：`.system-original.next`，相同模式。
- 同文件 `:816` 至 `:835`：启动恢复读取最终 repair-journal.json，不处理这三类孤儿文件。
- 同文件 `:667` 至 `:687`：completion 已用随机独占临时名，证明仓库已有可复用的局部模式。
- 全仓固定名检索只找到这三处写入与 `packaging/macos/local_install.py:1209` 的旧 managed 状态一致性检查；该安装器检查不是孤儿恢复。

**受限语义探针**

在临时目录预置三个同名文件，再用排他创建，各自收到 FileExistsError，旧字节不变。此探针只证明文件系统语义；没有执行 Rust 函数，没有通过真实 kill 构造窗口，也不证明实际用户已经遇到。

**边界修正**

固定残留阻塞对应的新写入。现有 pending 的 `recover_saved_plan` 使用 completion 与 clear，不写 system-original，因此不能说残留必然阻塞所有显式恢复。普通 legacy fix 也不等于这条兼容特权路径。采纳局部整改，不采纳扩大化影响描述。

**最小整改**

在 `crates/switcher-helper/src/platform.rs` 统一安全同目录临时写入。保留独占创建、root 锁、文件同步、目录同步与提交语义。历史固定残留只能在验证所有权、权限、类型、链接数及事务关系后处理，不能删除最终 journal/system-original 绕过错误。

**最小未来验证**

三个写入点分别覆盖创建后/写入后/rename前中断，再重试；验证最终记录不损坏。加入恶意链接与外部替换拒绝。补充已有 pending 可以恢复的反例，避免把兼容恢复删掉。现有 `platform.rs:432` 的 completion 残留测试只覆盖已修的 completion。

## F12：恢复失败使服务不可查询

**直接证据**

- `crates/switcher-helper/src/main.rs:29` 至 `:41`：先验证安装记录；非 source_local_pending 时获取 RootLock，调用 recover_pending_repair；`?` 直接跳过 xpc::serve 并退出。
- `crates/switcher-helper/src/platform.rs:837` 至 `:849`：服务有效性检查或第三方时区/IPv6状态冲突使自动恢复返回错误。拒绝覆盖外部修改是正确边界。
- `crates/switcher-core/src/install.rs:540` 至 `:546`、`:557` 至 `:572`：状态查询与显式恢复均需要 NativeClient XPC。
- `crates/switcher-helper/src/platform.rs:185` 至 `:198`：已有 recover_repair/repair_status 入口，但必须等服务已启动。

**验证状态**

当前仅静态控制流确认。未伪造 root journal，未启动 helper，未运行 XPC。复现条件必须同时包括：合法安装记录、非 source-local 安装 pending、合法旧 repair journal、自动恢复返回冲突或缺服务。不能宣称无 pending 的正常安装受此影响。

**最小整改**

将可恢复业务冲突与身份/权限/损坏记录错误分开处理。业务冲突下保留受限服务，允许查询诊断和现有固定恢复请求；新写操作继续拒绝。不要简单吞掉错误后启动完整写服务。预计变更 `crates/switcher-helper/src/main.rs`、`platform.rs`、`policy.rs`，并增加受限状态测试。

状态查询内部也会读取系统状态（`platform.rs:793`），缺失服务时可能再次报错；必须保留可查询的诊断错误，不能把“XPC可连接”当作“状态已可读/恢复已成功”。

**最小未来验证**

合成启动流程覆盖无 journal、可自动恢复、外部第三值、缺服务、无效权限/记录。冲突时 XPC 可查询且新写被拒绝；状态修复后仍走原授权与固定 journal 恢复。无 journal 分支行为保持不变。

## 建议实施顺序

```text
F02 卸载对象完整性 → F03 安装修复合同 → F07 首装选择与收尾
                                      ↓
                     F11 临时文件 → F12 受限恢复可用性
```

先补能够揭示跨模块差异的隔离夹具，再逐项最小修改。最后才对一个冻结候选统一构建和安装；本轮不执行这些步骤。
