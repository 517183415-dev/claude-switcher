# F-06：旧式账号事务的新进程恢复

## 本轮结果

在合成数据和隔离环境下，七个切点分别强制终止切换进程，再由新进程恢复两次。七个切点为 prepared、swapped、archived、committed、mirrored、active_cleaned 和 archive_cleaned。新进程检查目标账号、代次、事务清理、资料内容、文件权限和扩展属性；父进程另核对资料指纹及 B 归档身份。

[测试输出](test.log)记录一个顶层测试通过和七个子进程恢复成功。子进程结果不再计入顶层测试数量。执行结果为退出码 0；Rust 输出两条既有 ts-rs 属性解析提示。存档副本仅移除尾随空白；未修改的原始输出保留在 `output/no-account-20261027/f06-fresh-extra.2jplmX/test.log`，SHA-256 为 `f7558d74dae0de990e36647fa2b00b97d8dcfc0c7a937d7bf7040e34bed2d46f`。

```text
cargo test --offline --locked -p switcher-core --lib \
  accounts::legacy::tests::f06_other_switch_boundaries_recover_in_fresh_process \
  -- --exact --nocapture --test-threads=1
```

测试在 `sandbox-exec` 内运行，使用独立 HOME 与临时目录；沙箱禁止外网、默认 Keychain、真实账号目录和系统管理命令。测试前后的 `crates/switcher-core/src/accounts/legacy.rs` SHA-256 均为 `96922e4e7ee3df30fbf6fdc6c119a80501cb1409de472f53efc9a0ca4347434d`。执行基于 `ef37de24` 工作树，加本轮测试文件改动；并非对该提交的原始归档执行。修改文件的 rustfmt 检查通过。

代码图谱已更新：306 个文件、7561 个节点、30186 条关系；状态为 complete，待处理变更为 0。

原有 journal_written、active_exchanged 和 journal_unlinked 三个切点此前已通过新进程恢复测试；本轮复用同一测试逻辑，没有重跑这三个切点。相关既有证据见[此前进程恢复记录](../ordinary-next-index.md)。

本轮不证明断电、真实 Claude 登录态、普通 App 切换、实际安装或完整 F-06 验收。测试没有修改产品业务逻辑。
