# 整理历史的渠道提示回归

整理预览中的保留记录和移除记录共用 `history_label`。旧实现把两个渠道固定显示为“本机预览”或“本机正式”，即使记录来自远程目标。修复后只显示“预览”或“正式”，并保留序号和发行标识。

新增回归直接调用生产函数，分别检查预览和正式渠道。修复前的[失败日志](history-label-red-20261028.log)显示“本机预览”的错误提示；修复后的[通过日志](history-label-green-20261028.log)覆盖两个渠道。测试使用独立 HOME、离线 Cargo 和禁止网络及真实密钥库访问的隔离策略：

```sh
cargo test --offline --locked -p switcher-desktop --bin switcher-desktop cleanup_history_label_describes_channel_without_guessing_storage -- --nocapture
```

归档日志仅去除了行尾空格和末尾空行，原始日志保留在 `output/no-account-20261027/history-label-regression-r1/`。测试后只格式化了同一文件，未改变行为。Rust 格式检查与 Git 差异检查通过。没有重跑先前有效的完整测试，没有重建普通产品包；`afce48da` 候选包不含本次提示修复。测试没有运行真实 R2 或窗口中的手动操作。
