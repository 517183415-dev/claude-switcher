# A01/F15 本地整改证据

结论：限制不一致已修复。4 个源文件变更。新增 Rust 4/4、UI 5/5、既有备份 1/1；10 个独立方法全部通过。真实 IPC/WebView、Keychain、原生 GUI 均 NOT_RUN。本结果仅覆盖隔离本地处理链。

## 根因与最小修复

- `publisher_commands.rs:42` 图片 gate 现在复用 `switcher_core::ads::MAX_IMAGE_BYTES`（1 MiB）。
- `publisher_commands.rs:192` 生产命令进入 `spawn_blocking`，调用 `:209 restore_publisher_backup_body`。该 body 先按 `publisher::backup::LIMIT`（64 MiB）拒绝超限，再包裹口令 Zeroizing，再调用注入 restore callback，最后建立原 OperationOutcome。
- 生产 callback 仍为 `with_publisher → Publisher.restore_backup → KeychainVault`。测试 callback 只替换 Publisher/存储边界为临时目录/MemoryVault，真实核心恢复逻辑不替换。size gate 位于 `with_publisher` 前；Tauri 参数反序列化和线程调度没有被该测试覆盖。
- `Operations.svelte:34` 图片 gate 改为 1 MiB；`:40` 备份提示改为“不超过 64 MiB”。格式、KDF、版本兼容、备份核心与 publisher/mod.rs 字节均不变。

## RED 与 setup 区分

- `rust-red.{stdout,stderr,json}`：1 方法失败，exit 101，54.759 秒。实际 33 MiB 合法密文经过 production 共用 body，被旧 32 MiB gate 拒绝。失败原文：`备份文件超过 32 MiB 上限`。源冻结为 true。
- `ui-red.*`：5 方法，3 过/2 败，exit 1。真实 imageInput 将 1 MiB+1 文件 invoke；真实 restoreBackup 的错误提示仍为“小于 64 MiB”。
- `rust-setup-no-lib.*`：错误选择 --lib（包为 binary target），未进入测试，不算 RED。
- `rust-setup-fixture-arity.*`：测试 fixture 少传 now 参数，编译失败，不算 RED。修正夹具后取得上述真实行为失败。
- `red-*` 是 RED 时源码快照；`baseline-*` 是开始前快照。

## GREEN 与场景回执

- `rust-green.*`：4/4，exit 0，193.792 秒（方法执行177.29秒）。过滤器仅 `f15_`；其他4个 integration test binary各0方法运行，不计方法数；没有运行 e03 updater/sysctl 方法。后续命令显式 `--bin switcher-desktop`。
- `publisher/tests.rs:820` 以真实 export 生成合法 snapshot，经原 open 加入合法 JSON 尾空白，再按未修改的 seal/KDF 加密。`:871` 33 MiB、`:876` 精确64 MiB均进入实际 shared body 并恢复，验证新 generation、MemoryVault存入、草稿素材相同与重开目标恢复一致；源 active pointer/generation/vault保持不变。
- 这些大文件是有效格式的 padding 样本，不是大量真实素材负载。长度含60 B header与16 B tag；`:953` 超出 plaintext预算1字节被seal拒绝。
- `:881` 错口令、篡改、截断、64 MiB+1、MemoryVault store失败均经同一body拒绝；超限在callback前拒绝。每次检查目标generation/draft/active pointer/MemoryVault与源active pointer/MemoryVault不变。晚期vault失败允许既有实现留下未引用generation文件，本测试不声称目录逐字节无变化。
- `ui-green.*`：5/5，exit0，1.641秒。直接提取并执行真实 Svelte imageInput/restoreBackup 函数，mock invoke。1 MiB-1、1 MiB图片接受；1 MiB+1、8 MiB在读取前拒绝。64 MiB备份metadata接受；64 MiB+1与短口令在读取前拒绝；backend失败不伪报成功并释放busy。File metadata和4字节arrayBuffer均为合成，不能作为64 MiB WebView序列化证据。
- `rust-existing-backup.*`：既有加密随机性/口令/KDF参数/atomic generation恢复方法1/1，exit0，32.424秒。
- `rust-clippy.*`：offline/locked指定binary，`-D warnings`，exit0，2.988秒。仅保留既有ts-rs deny_unknown_fields解析提示。
- `fmt.*`、`whitespace.*`：exit0。初次直接rustfmt写入/检查未覆写HOME；已保留 `fmt-initial-no-home-override.*` 并使用合成HOME重做最终只读检查。未通过rustup代理、未安装/下载；不把初次formatter环境记录写成已隔离。

## 可复现命令

`run_check.py` 将绝对 cargo/rustc/rustdoc 路径固定为 1.96.0-aarch64-apple-darwin，固定 CARGO_HOME/RUSTUP_HOME/RUSTUP_TOOLCHAIN、CARGO_NET_OFFLINE=true 和合成 HOME/TMPDIR。每次收录完整命令、stdout、stderr、exit、耗时与源前后hash。

```text
/opt/homebrew/bin/python3 -B output/review/A01-F15-整改-20261025/run_check.py rust-green test --offline --locked -p switcher-desktop f15_ -- --nocapture --test-threads=1
/opt/homebrew/bin/python3 -B output/review/A01-F15-整改-20261025/run_check.py rust-existing-backup test --offline --locked -p switcher-desktop --bin switcher-desktop publisher::tests::encrypted_backup_randomness_password_parameter_limits_and_atomic_generation_restore -- --exact --nocapture --test-threads=1
/opt/homebrew/bin/python3 -B output/review/A01-F15-整改-20261025/run_check.py rust-clippy clippy --offline --locked -p switcher-desktop --bin switcher-desktop -- -D warnings
/opt/homebrew/bin/python3 -B output/review/A01-F15-整改-20261025/run_ui.py ui-green
```

UI runner仅引用R07既有APFS镜像的typescript依赖，临时复制两个被测文件并只读使用缓存。未访问原checkout业务文件。没有 C2C、外部助手、网络/回环、真实产品、账户、系统服务、Keychain或GUI调用。临时目录随runner退出清理，无留存运行进程。

## 冻结 SHA256

| 路径 | SHA256 | 相对A01基线 |
|---|---|---|
| `apps/desktop/src-tauri/src/publisher_commands.rs` | `40946ebe38223275dd4799a4f216b22b0e84d45c7b4bd5bbf4dde5d6ac8692b0` | 变更 |
| `apps/desktop/src-tauri/src/publisher/backup.rs` | `5e3d4ca7ca42c160cc0ed311d6fb62117977ebaccbf0383da36a2191c01e7f68` | 未变 |
| `apps/desktop/src-tauri/src/publisher/mod.rs` | `67ca13a152abcdb5a46a64d041fb8b94307d91c246098fa0eeab11f747f8cc2d` | 未变 |
| `apps/desktop/src-tauri/src/publisher/tests.rs` | `d4ff8bb7f1c2fbb3f0c0195daacda1ab5359cd795ba6545ab5134ba349834ebe` | 变更 |
| `apps/desktop/src/pages/Operations.svelte` | `75399b751448be483e876acc817d66c7d7fb37e529042fa1b163cf9861daf87b` | 变更 |
| `apps/desktop/tests/publisher-limits.test.mjs` | `f3c1c3882c7fa814fc7efe698f33f16c6d7a0f102e793cd2b7727847c167f663` | 变更 |
