# A02 核心独立审查最终结论

结论：ACCEPTED_LOCAL_CORE。唯一 P2 已闭环；本次范围未发现剩余 blocker、P1、P2。

mod.rs:1251、1298 显式使用 restored_key 校验备份状态；validate_resolution_with_key 使用此公钥验证两份签名。验证期间不修改 self trust；trust 写入仍在验证之后。

tests.rs:1567 创建全新目标。恢复前断言 public_key=None 且 trust.json 不存在；恢复后和重启后均比较完整终态证据。真实 RED 为 MissingSigningKey；同项 GREEN 通过。最终 rust-core-final 30/30、exit0。

冻结 SHA256：

| 文件 | SHA256 |
| --- | --- |
| publisher/mod.rs | 90f0f535908eaa229c00383fcbc86c569744612c9c3f42aaa0c7b596ce7189a6 |
| publisher/tests.rs | 98d631952fc227ead9168192c29474a8c88f6c0296ca3622cbbf63ec84de7462 |
| publisher/protocol.rs | a1302532121908867d8b7a07ca3c773ca483235b6c428361a81ea316a4f1ada3 |

审查者：本地独立 agent /root/f01_review。基于只读源码和已保存回执；未重复执行测试。命令/UI、真实 Keychain、IPC、远端运行不在本轮结论内。后续 A03 将另行改动 mod.rs，以上是 A02 接口冻结检查点。
