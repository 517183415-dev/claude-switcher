# R10 / F13 只读调用定位

状态：只读映射完成；未修改业务源码，未运行测试、cargo、rustup、产品或系统入口。没有 C2C、浏览器、网络或外部助手调用。以下是当前源码事实与未来验证建议，不是运行验收。

## 1. 实际归属与调用链（直接证据，高置信度）

最小整改计划第93、96行要求先确认对应用户文件。`apps/desktop/src-tauri/src/guardian.rs` 不存在。实际 guardian 位于 switcher-core，不能按计划里的旧路径改 helper。

```text
packaging/macos/launchd/local.claudeswitcher.guardian.plist:5
  /Library/Application Support/claude-switcher/switcher-guardian
    → crates/switcher-core/src/bin/switcher-guardian.rs:57
      guardian::run_native
        → crates/switcher-core/src/guardian.rs:1083 / :1089 / :1093
          guardian::run
            ├─ :534 timezone_guard::spawn(state_path, shutdown.clone())
            │    → crates/switcher-core/src/timezone_guard.rs:90
            │      Scheduler::tick_at (:18), spawn_blocking (:41)
            │        → reconcile (:63) → systemsetup -settimezone (:75–79)
            └─ :536 run_driver (...), await (:554), timezone_guard.abort (:555)
```

- 用户模块注册只有 `crates/switcher-core/src/lib.rs:29` 的私有 `mod timezone_guard;`。限定生产树搜索中，spawn 的调用点只有 guardian.rs:534；abort 在:555。
- binary :27–39 校验当前 uid 与 root install record owner，uid 0 或其他 session 不进入 run。因此该路径按当前代码是用户 session 调用，不是 root watchdog。
- 用户时区模块 :50–59 读取固定 v2/config.json，验证时区后调用 closure。:65–74 先读 `/etc/localtime`；不匹配才直接启动 `/usr/sbin/systemsetup -settimezone`。:75–85 没有 helper/XPC 授权调用，且只看命令退出状态。
- :7 为3600秒间隔；:8 为5秒poll/retry。:29–36 仅对两个特定错误码选择5秒重试，其余完成结果进入小时延迟。开头“blocking XPC call”注释与当前直接命令实现不符。
- 以上只证明源码路径存在。未执行该用户命令，未证明本机命令成功、频率或与root进程发生实际竞争。

## 2. 必须保留的职责（直接证据）

| 职责 | 当前锚点 | 本单元边界 |
|---|---|---|
| root 时区看门狗 | `daemons/tz-watchdog.sh:6–13`；`daemons/local.tz-watchdog.plist:6–8` | root固定路径、RunAtLoad、3600秒均保留。仓库模板不代表本机已注册。 |
| helper 授权时区实现 | `crates/switcher-helper/src/platform/timezone_guard.rs:13–48`；platform.rs:111–112 | 保留校验、get/set/get回读及策略入口；不是拟删除的用户模块。 |
| guardian健康/安装握手 | `crates/switcher-core/src/guardian.rs:535,551,738–739,768–779`；`installation_health.rs:121–155` | 保留GuardianProbe、timer sequence、heartbeat和单飞health调用。 |
| 网络/唤醒事件与fallback | guardian.rs:733–749、:942–957、:1088–1094；FALLBACK_INTERVAL :5 | 保留事件加速及10秒fallback；不改native observer注册。 |
| 受信Public调度 | guardian.rs:740–745、:1162–1198 | 保留60秒调度探查、owner/public/developer_team门槛、codesign验证与install record复读。SourceLocal不会穿过developer_team要求。 |
| 停机执行者 | guardian.rs:128–149、:153、:703、:720–729、:767 | 不改evaluate、StopSupervisor或stop_verified_claude。不尝试合并root/bodyguard用户停机路径。 |

## 3. 最小未来改动建议（本轮未做）

1. `crates/switcher-core/src/guardian.rs`：删除run里的timezone spawn/abort及仅为其保留的result临时变量；直接返回原run_driver await。run_driver、health hook、events、Public调度、stop逻辑字节保持不变。
2. `crates/switcher-core/src/timezone_guard.rs`：删除整个私有用户校时模块。其当前3个测试证明旧校时路径，不应继续假装是保留合同。不要把实现替换成定时空转任务。
3. `crates/switcher-core/src/lib.rs:29`：删除对应mod注册。这是完成模块删除所需的第三个文件，不涉及helper模块或API。若范围最初仅允许两文件，先由父会话明确纳入此一行。

本映射未建议删除binary、LaunchAgent、root watchdog、helper授权接口或任何stop执行者。

## 4. 测试形态及已有选择器（全部 NOT_RUN）

现有用户校时测试在 timezone_guard.rs:158、:172、:182，分别为目标读取并调用writer、非法目标不调用writer、启动后小时调度。当前这些测试都不调用真实systemsetup；其存在不证明生产用户路径有权限。

新的边界证据应区分：

- **静态删除合同**：源码扫描证明用户guard生产入口不再引用timezone_guard或systemsetup，模块注册已删，root/helper源码hash不变。可以在已有guardian测试模块加入限定生产source的结构回归，RED应是断言失败而不是缺文件编译失败。
- **保留行为合同**：继续使用已有run_driver的假check/stop/health hooks，验证timer与Wake/NetworkChanged仍驱动checks/health，真实health JSON仍写入隔离temp根，timezone failure不触发stop。此层不会调用native_events或真实checks。
- **无误称**：现有run_driver测试绕过run wrapper，不能单靠其GREEN证明旧timezone spawn已移除。删除与调用图证据必须同时给出；若要求行为RED直接覆盖组合入口，应先做可注入、行为不变的最小wrapper seam，不能执行原生run去试错。

可复用的精确测试入口：

```text
cargo test --offline --locked -p switcher-core --lib guardian::legacy_behavior_tests:: -- --nocapture
cargo test --offline --locked -p switcher-core --lib installation_health::tests:: -- --nocapture
cargo test --offline --locked -p switcher-core --lib guardian::native_tests::network_epoch_native_queue_overflow_still_invalidates_every_event -- --exact --nocapture
cargo check --offline --locked -p switcher-core --lib --bin switcher-guardian
cargo clippy --offline --locked -p switcher-core --lib --bin switcher-guardian -- -D warnings
```

这些是未来命令建议，不是本轮执行记录；应使用父会话已验证可用的离线工具链，禁止触发rustup下载。改动后另对拥有文件做rustfmt check / git diff --check。

`guardian::legacy_behavior_tests::` 的4项现有测试：

- :1292 `only_proxy_or_tun_failure_requests_stop_for_every_event`
- :1310 `legacy_path_scope_sends_term_once_per_matching_process_without_signature_gate`
- :1317 `legacy_shell_only_configuration_loads_at_startup_and_refreshes`
- :1394 `driver_checks_before_stop_and_events_config_or_heartbeat_cannot_stop`

禁止在本轮以宽泛 `guardian::` 过滤代替精确选择：`:1117 network_epoch_native_receiver_keeps_observer_alive_and_drop_is_detectable` 与 `:1130 read_only_native_event_registration_and_cleanup` 会调用真实native_events。上述queue_overflow单项只用合成channel与atomic callback。

Public调度处于 `cfg(all(target_os="macos",not(test)))`，不由这些单测实际执行。保留原代码/hash并编译该生产分支，可证明未误删，不能声称实际Public任务已运行。root watchdog/helper同理保留只读证据，不能补写真实系统PASS。

## 5. 当前源码 SHA256

| 文件 | SHA256 |
|---|---|
| `crates/switcher-core/src/bin/switcher-guardian.rs` | `741904da07e84c2bd965293f873706e2d7e4769e56d325fa9da99c38be6a5f38` |
| `crates/switcher-core/src/guardian.rs` | `06624720b2d5af336eb2d3003a13fa0e98b0bfe8a5260145fa94cec8200ceecf` |
| `crates/switcher-core/src/timezone_guard.rs` | `ab74aec0205104b27f4daf7d8f080d6dbf52d648655fd78b6465bf2ce3f2a8d6` |
| `crates/switcher-core/src/lib.rs` | `1c479bd509517d5f4f4f796d8de50baf52d7fb9356f89b0d0afe7880913fd872` |
| `crates/switcher-core/src/installation_health.rs` | `f9bd81d6a274f27be221af9d5d80c5df73ce5a524ae031c13dca0b45fe163d78` |
| `crates/switcher-helper/src/platform/timezone_guard.rs` | `8d328bfc625a8a6e335037c7e7cbbbc11570507575dee7dbaf41c095e3f7f914` |
| `daemons/tz-watchdog.sh` | `4f8d899ab0babecb9983d88d58e0f1feb13828dbad4600219cead1cc42f90916` |
| `daemons/local.tz-watchdog.plist` | `c0b6b95e88becfb9aa51ec75e3c2a04b8e5a8a2698c5f87ec139d57621971300` |
| `packaging/macos/launchd/local.claudeswitcher.guardian.plist` | `995b0c77c1750fe4b4b8ddb9b5a0699a1252dffbbbb58b3ad0d29d28bf588cf2` |
