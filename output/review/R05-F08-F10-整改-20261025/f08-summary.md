# R05 / F08 执行证据

任务 c2c_824e，F08 独占源码 `crates/switcher-core/src/macos.rs`。本切片仅改该源码及本目录 f08-* 证据。源码已冻结，最终 SHA256：`7eb6337c6fed628460ffe8bd38da719f2f86aa676fd0cc5615693c1f8208c5c3`。

## 变更与边界

`read_command:128` 仍对所有既有通用调用者使用固定两秒。其内部实现抽取到 `read_command_with_timeout:132`。生产 DNS 固定入口 `dns_addresses:312` 使用 `/usr/bin/dig`；私有 `dns_addresses_with_program:316` 以原有 `+short,+time=3,+tries=1` 参数和四秒 observation timeout 调同一实现。

保留 env_clear、固定 PATH/LC_ALL、stdin null、stdout piped、stderr null、256 KiB 上限、UTF-8与exit检查、kill_on_drop、timeout后kill().await再wait().await。首行DNS答案合同未改。没有新增生产可配置程序路径。

只读 test-local TIMEOUTS 回执记录实际自建child PID及真实kill/wait结果；它不返回预制观察结果、不短路spawn/read/wait。四个新方法没有使用 OUTPUTS 模拟返回。

## RED

`f08-red.log`、`f08-red-receipt.json`、`f08-red-source.rs`：先参数化seam，DNS常量仍2秒。实际命令：

```sh
cargo test --offline --locked -p switcher-core --lib f08_ -- --nocapture
```

编译成功；4方法，3通过、1行为失败，exit101。唯一失败是 `f08_dns_accepts_real_delayed_child`：计划sleep2.5秒后返回synthetic IP的真实child，在 **2.001766375秒** 返回 `observation_timeout`，成功IP断言失败。generic两秒与错误/超时分支当时通过。

前两次探索日志单列：`f08-scaffold-compile-failure-*` 为sha256日志LowerHex格式不兼容，已修正且不计RED；`f08-red-mixed-fixture-collision-*` 同时有DNS行为RED和并发临时目录命名碰撞，已用进程内原子计数器消除碰撞，最终使用上面的clean RED作为证据。

## GREEN

仅将 DNS timeout 常量2→4秒后，同一cargo命令通过：**4方法、10真实fake-child子例**，0失败/忽略，exit0；总测试耗时8.05秒。`rustfmt --check --edition 2024 crates/switcher-core/src/macos.rs` exit0。准确命令、退出码、源码前后hash见 `f08-green-receipts.json`。本切片未重复父端全模块/clippy矩阵。

| 场景 | 实测结果 |
|---|---|
| DNS sleep2.5秒，写synthetic IP后正常退出 | 2.544807750秒，`Ok([198.18.0.1])` |
| 相同2.5秒child走通用read_command | 2.005077625秒，timeout，kill_ok=true，wait_ok=true |
| DNS child持续保持stdout并sleep30秒 | 4.003568792秒，timeout，kill_ok=true，wait_ok=true |
| DNS child先关闭stdout，再sleep30秒 | 4.002679625秒，timeout，kill_ok=true，wait_ok=true；证明预算覆盖wait阶段 |
| 非UTF-8 / 非零退出 / 256KiB+1 | 分别observation_invalid / observation_unavailable / observation_too_large |
| 首行CNAME、次行IP | dns_observation_unavailable，不能跳过首行 |
| 两行IP，stderr另写文本 | 只返回首行IP；stderr不进入结果 |
| 恰好256KiB | 通用runner成功返回全部bytes |

超时回执中PID均来自对应fake-child自己的临时pid文件，测试同时断言回执PID一致。日志记录实际kill和wait返回成功。没有枚举、终止或等待无关PID。

四秒是 observation future 的deadline；上述实测包含少量清理/调度时间，**不承诺kill/wait也处于绝对墙钟四秒上限内**。取消保留原kill_on_drop合同，本单元未扩展通用进程组管理，也不声称主动取消后已显式reaped。

## 假程序与证据范围

假dig使用现有 `/opt/homebrew/bin/python3` 的绝对shebang（`-I`），不产生shell/sleep孙进程。内容只有本机临时pid文件、环境/stdin断言、time.sleep与synthetic stdout/stderr；不导入socket，不调用真实dig/curl，不访问DNS/网络。DNS延时正例还断言真实传入的四个dig参数与host fixture.invalid。脚本模板保存在源码快照中；每个实际fake-child源码SHA256和body见 `f08-fake-child-manifest.json`，对应原始stdout见 `f08-green.log`。

没有安装、依赖下载、真实DNS、外网、真实账号、应用、launchd或系统设置操作。此结果仅证明隔离进程预算与数据处理合同，不证明真实DNS可达或整机网络验收。
