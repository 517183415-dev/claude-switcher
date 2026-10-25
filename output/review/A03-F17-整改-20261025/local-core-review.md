# A03 核心独立复核

复核者：本地只读子代理 `audit_ads`。最终静态结论：冻结四文件未发现剩余 P1/P2。复核者未执行测试，无可用 LSP 诊断工具；这是静态无阻断结论，不是产品运行验收。

## 保留的复核发现

第一轮 `REQUEST CHANGES` 确认一项 P2：单候选在 unlink 后中断，重启预览仅剩 already_missing 时，可跳过两个目录的同步并清除批次。最终实现于清批次前无条件同步两个目录；同步失败保留批次。真实目标断言 RED 及修复后 GREEN 均保留。

另一个容量候选经合成回归确认：近 16 MiB 的合法状态可创建未预留完整 GC 身份的批次。最终在检查点写入前和保留活动批次的 persist 门槛检查最坏编码空间。上界按最多 128 个身份、整数最大十进制长度、最长分类、false、固定 hash 和槽名计算。open/validate_state 的既有读取门槛不变。合成 padding 证明编码门槛缺口，不证明普通运营自然达到该容量。

## 最终核对

1. 目录 FD 与 EXCL 隔离防止覆盖隔离槽。唯一 unlink 指向隔离槽，重新创建的原 hash 路径不被删除。
2. 隔离后重验 dev/ino/owner/mode/nlink/size 和完整 hash。异常保留批次与证据。
3. 两份 current 共用 publication 锁；认证水位、固定检查点绑定、两次确认、备份瞬态字段剥离和活动目标恢复阻断保留。
4. 最终 19/19 日志已核对。默认保留 128 的满历史、满资源回归通过。测试是注入故障与重开，不等同实际 SIGKILL 或断电。

威胁边界：协作进程遵守 lifecycle/publication 锁，私有隔离目录没有其他产品 writer。不承诺抵御恶意同 UID 修改目录或进程。

| 文件 | SHA256 |
| --- | --- |
| apps/desktop/src-tauri/src/publisher/mod.rs | 39557bcbbb41abc50234c7f29e77df5f46af93ac5c042fe2afd2efa803f7a5d0 |
| apps/desktop/src-tauri/src/publisher/local_store.rs | bf0d23ec4018a27043f503756405cf433db78f6f5993376a89df28524629f1a5 |
| apps/desktop/src-tauri/src/publisher/cleanup.rs | 3d4d31c97499fc200dabe1dd4bd88bef0583127f34e0b93306ca03b9464d930f |
| apps/desktop/src-tauri/src/publisher/cleanup_tests.rs | 2e9e855a604fe896e1ee8c8b4ed71c8ecda32811de65a74c931abbb80e92be6e |
