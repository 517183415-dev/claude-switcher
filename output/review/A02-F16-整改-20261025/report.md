# A02 / F16 核心实施与隔离证据

已冻结核心源码。只改 `publisher/mod.rs` 与 `publisher/tests.rs`；`protocol.rs` 与原有 A01 测试原文保持不变。最终哈希见 `final-hashes.json`，完整副本见 `accepted-sources/`。

1. pending 恢复先认证 source.current 与 replay watermark；公开 exact-wire 不能绕过 source 错误。exact current 先保存提交水位，再补 immutable version/assets，不再次 CAS。坏 immutable bytes 不覆盖。
2. typed `ResolutionKind` / `ResolutionSummary` / `Resolution`、`Error::Resolved(summary)`、`Status.last_resolution` 和 `resolution_read()` 提供后续命令映射与 A03 引用保护。三个非 verified 终态均保留原 pending wire、认证 current wire/etag、时间、水位与缺失素材标识，不写 verified history。
3. 单槽证据不会被普通保存、打开、成功发布或旧备份恢复清除。第二终态保留新 pending 并返回 `ResolutionOccupied`。同一退休操作重启重试返回相同摘要，不隐式创建新发布。全局 pause 清空 campaigns，可发布更高序号安全内容。
4. persist 保留 IO 错误，同时在 rename 后错误时用 exact 磁盘字节校准内存。测试使用 cfg(test) 私有注入：一次在实际 atomic_write 前返回错误；一次完成真实 atomic_write 后模拟目录同步错误。此证据验证提交点错误处理，不声称真实磁盘/fsync 故障复现。

## 真实 RED 与 GREEN

所有 Cargo 命令的精确绝对 argv、固定离线环境、隔离 HOME/TMPDIR、source_before/after、exit 和耗时均保存在相应 `.json`；原始输出保存在 `.stdout` / `.stderr`。

| 记录 | 方法 | 结果 | 意义 |
| --- | ---: | --- | --- |
| rust-red | 6 | 0通过、6行为失败，exit101 | unchanged production 的真实 RED，无 setup/compile 失败 |
| rust-green-initial | 6 | 全通过，exit0 | 首轮最小修复 |
| rust-green-expanded | 15 | 全通过，exit0 | 认证、unsafe、原子错误与持久绑定矩阵 |
| rust-red-retired-retry | 1 | 1行为失败，exit101 | 首轮修复仍隐式重发退休操作的缺口 |
| rust-green-f16 | 17 | 全通过，exit0 | 新增退休幂等与小备份兼容后 |
| rust-core-regression | 29 | 全通过，exit0 | 第一轮格式化源码；精确 `--bin switcher-desktop publisher::tests:: -- --skip f15_` |

独立审查补充：初轮无 trust 新 Publisher 恢复带终态备份时错误依赖当前实例公钥。新增 `f16_clean_publisher_restores_resolution_using_authenticated_backup_key` 真实 RED（MissingSigningKey，exit101），随后用已验证 restored_key 显式验证 snapshot/pending/resolution，未提前修改 self trust。相同方法 GREEN exit0。最终 `rust-core-final` 为30/30，exit0，source_frozen=true。对应精确记录 `rust-red-clean-resolution-restore.json` / `rust-green-clean-resolution-restore.json` / `rust-core-final.json`。

F16 为 18 个方法、41 个输入场景。独立输入计一次，内部连续调用、两次重启、断言数量不当作额外测试。最终 30 方法包含全部 18 F16 与 12 个既有 pure core 回归。A01 f15_ 大密文沿用既有接受证据，未重新运行；本次新 State 字段通过小备份 export/restore 专项验证。

rustfmt check 与 diff whitespace check 均 exit0，见 `format-check.json`、`whitespace.json`。现有 ts-rs serde 属性告警保留；初轮新 API 在生产构建中的未使用告警属于尚未进行的命令/UI接线，未通过修改调用方消除。

## 未运行范围

真实源/网络/DNS、真实 HOME/账号、原生 IPC/Keychain、GUI/服务、命令/UI接线、full desktop/e03 updater 均 NOT_RUN。只使用 MemoryStore/MemoryVault、LocalRoot 临时文件与现有 local-store 的临时根；不运行 publisher 其他子模块测试。完整范围与方法名单见 `verification.json`。

后续 A03 必须保护 Resolution.pending.manifest 与认证 current wire 所承载 manifest 的素材引用；释放槽必须有显式预览与用户确认。本切片没有增加自动清除入口。
