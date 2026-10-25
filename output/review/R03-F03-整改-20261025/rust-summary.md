# R03 Rust 执行证据

- 唯一修改源码：`crates/switcher-core/src/guardian_repair.rs`；最终 SHA256 `de4a3c3b0829f30a99b3d5b5ca3524ed8c04917ec6bd0ab07d901bb8a3fac67d`。源码已停止写入。
- `validate_root_plist:340` 只接受实际 plistlib canonical 编码和对应 bundled 模板。`read_root_file:416` 检查 protected parents；`read_checked_root_file:394` 用同一 NOFOLLOW/NONBLOCK FD 检查元数据并限长读取。UID/GID=0、plist=0644/script=0755、regular/nlink=1。`legacy_admin_commands:429` 即使已注册也验证组件；健康已注册不生成 bootstrap。
- `plan_watchdog_update:813` 检查唯一严格有效 TARGET；变更生成双引号，历史无引号同目标 no-op。`legacy_watchdog_command:804` 使用该规划器。测试桥 `legacy_tests::f03_emit_watchdog_update:733` 使用约定 env JSON；env 缺省只做纯规划断言，无固定守护路径访问。
- 用户 Guardian 原生产块 `safe_file` 至原 tests 之前与 HEAD 逐字一致（已脚本断言）。未改依赖、daemon 模板、Python 或其他生产文件。

## RED

`rust-red.log` / `rust-red-receipt.json` / `rust-red-source.rs`：4 方法，1 通过、3 行为失败，cargo exit 101。失败为 actual Python plist 拒绝、registered corrupt 错误 ready、TARGET 未带引号。编译成功，不把编译失败算 RED。

Python agent 另捕获跨语言 C RED；Rust planner 在其确认前保留旧渲染。其冻结 pre-F03 Python 真实验证拒绝 unquoted 更新，见该 agent 的 `python-crosslang-pref03-red.log`。

## GREEN

命令与前后源码哈希见 `rust-green-receipts.json`，全部 exit 0：

1. `cargo test --offline --locked -p switcher-core --lib f03_ -- --nocapture`：6 方法。
2. `cargo test --offline --locked -p switcher-core --lib guardian_repair::`：17 方法，包含以上 6 方法，不相加。
3. `cargo test --offline --locked -p switcher-core --lib regions::`：51 方法。
4. `cargo clippy --offline --locked -p switcher-core --lib --tests -- -D warnings`：通过；现存 ts-rs 宏输出 deny_unknown_fields 提示。
5. `rustfmt --check --edition 2024 crates/switcher-core/src/guardian_repair.rs` 与 `git diff --check`：通过。

随后只移动函数注释。最终源码再次运行 `cargo test --offline --locked -p switcher-core --lib guardian_repair:: -- --nocapture`，17/17，exit 0；最终哈希前后相同，见 `rust-final-guardian-receipt.json`。

子例独立计数：4 注册组合；两种服务合计 26 恶意 XML 变体，各覆盖已注册/未注册，合计 52 拒绝组合。文件测试覆盖真实隔离文件 mode、symlink/dangling symlink、hardlink、目录、限长与路径替换后仍读原 checked FD；仅 UID/GID 通过窄参数模拟，生产元数据谓词保持执行。没有以 unconditional success 绕过内容校验。

## 真实跨语言产物

Rust 测试用 `/opt/homebrew/opt/python@3.14/bin/python3.14` 导入当前 `packaging/macos/local_install.py`，调用真实 `watchdog_payload`/`bodyguard_payload`，保留 production path 字符串，然后交给生产 `validate_root_plist`。解释器、Python source hash 和生成 bytes/hash 均记录在 nocapture 日志。

- watchdog plist SHA256：`c2cd59a3520ee1fcc5f7bf22f43a9490ceba1c5598510209fa77452f049539de`
- bodyguard plist SHA256：`e817da557237d2df6f1b2d7cef20db6c88445df1fad81d38445ca8c506fa76d8`

未安装、未操作宿主服务/设置/真实账号；未执行守护脚本。以上是隔离回归，不宣称 native set-region 或真实 launchd 验收。
