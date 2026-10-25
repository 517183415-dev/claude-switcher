# A02 / F16 核心计划

状态：只读预检完成。等待父会话明确“文件释放”，然后写行为 RED；当前没有改源码或运行产品、Cargo、原生接口。

## 1. 所有权与现有缺口

只修改三个文件：

- `apps/desktop/src-tauri/src/publisher/mod.rs`：状态、认证、恢复、终态、无素材安全暂停。
- `apps/desktop/src-tauri/src/publisher/protocol.rs`：仅必要的 pending 提交事实字段；优先把终态类型留在 mod.rs。
- `apps/desktop/src-tauri/src/publisher/tests.rs`：内存 store/vault 与临时目录回归。

不修改命令、UI、配置、依赖或其他 agent 文件。A01 尚拥有 tests.rs，释放前不写源码。

CodeGraph readiness 为 complete；已完成通用和任务查询。当前定位：`read_remote:584` 已验证签名、scope、issued_at 非 future，并通过 Sequence.remember 检查回退/冲突；`publish_with:741` 先读公开 current，随后 exact-wire 直接进入 `finish_readback:796`；`verify_public_readback:803` 要求未过期且所有公开对象齐全，缺少恢复补齐；`pause:431` 只 disabled，仍携带 campaigns；`persist:301` 复用原子写并仅在成功后更新内存。

## 2. 有界终态字段

优先 `#[serde(default)] last_resolution: Option<Resolution>`，兼容旧 State。Resolution 保存原 pending 全部字段（尤其 exact wire）、认证 current 的 exact wire/etag（允许已证明 None 的显式表示）、kind、resolved_at 和对应水位证明。kind 区分 superseded、expired_committed、committed_assets_unavailable；任何此类结果都不是 verified publication。

普通 open、draft_save、pause、成功 publish 和 backup/restore 不得自动清除/覆盖该证据。第二个终态若槽已占用，返回容量错误并保留该次 pending。后续命令/UI必须通过显式用户预览和确认，才能决定归档/替换旧证据；本切片不自行增加该流程。已有证据相同的重试可幂等返回原摘要。

此方案安全且有界，但不会假装可以无限自动退休 pending。A03 引用保护必须扫描 last_resolution 中原 pending 与认证 current 的 manifest 素材；不能只扫描 draft/history/pending。若后续要求无需显式动作自动保存多个终态，应先单独批准有界日志设计。

validate_state 重验终态 wire 的签名、key/channel、pending manifest 绑定、current 与 pending 的 kind 关系、issued_at <= resolved_at、水位证明与当前水位不回退。不可只信本地 summary 或 phase。缺 key 时不能把未认证证据认作有效终态。Resolution/kind/summary 使用 typed 可见类型，后续命令/status直接映射，不解析错误字符串。父会话已认可单槽与后续 A03 显式预览确认释放合同。

## 3. 最小恢复流程

```text
pending -> authenticated source.current + Sequence.check
        -> exact wire: durable commit fact/watermark -> repair immutable objects -> public verify
        -> higher trusted revision: atomic superseded evidence + retire pending
        -> expired committed / proven missing assets: atomic nonverified evidence + retire pending
        -> invalid / unknown / unsafe: retain pending, return error
```

所有 pending 恢复首先调用共用认证逻辑，不以公开 current 当提交证明。exact-wire 必须先持久化提交事实/水位，然后只补 create-if-absent immutable version/assets，不再 CAS。已存在坏字节返回错误，不覆盖。公开回读全部成功且尚未过期才进入 verified history。

仅远端明确 None 且本地 `Error::Io(NotFound)` 才把素材视为不可补齐；missing asset index、坏图片、symlink/hardlink/unsafe mode、权限、普通 IO 与远端未知均保留 pending。expired 已认证 committed 可保存独立非 verified 终态。更高可信 current 只推进水位并保存 superseded，不改 remote。future、坏签名、回退、同序不同内容、有水位但 current 缺失均 fail closed。

终态与 pending retirement 必须在同一次 persist 中原子保存。atomic_write 的 rename 后目录 fsync 仍可能返回错误；不得假设 persist Err 等于磁盘未提交。参考 restore_backup 的提交后读盘校准，在这一终态边界验证实际磁盘 State，维持内存/磁盘一致并保留明确错误语义。typed `Error::Resolved(summary)` 可保留现 Result API，留给后续命令适配 `completed=false`。普通重试错误不得转换为 Resolved。全局安全暂停将全 slots disabled 且 campaigns=[]，并通过正常更高 revision 保存/发布，不依赖丢失图片。

## 4. RED -> GREEN 验证矩阵

先新增 `f16_` 行为测试，在旧生产上真实 RED，保存完整 command/env/source hash/exit/stdout/stderr；确认失败来自行为而非编译。随后实施最小修复并运行相同过滤器 GREEN，再运行 pure publisher 全部测试。沿用 MemoryStore，添加最少故障注入和访问/CAS计数。

- 丢提交 reply 后重启、跨过期时间；缺 version、缺图片（本地有/NotFound）、exact current 无重复 CAS，非 verified 终态不进 history。
- 更高认证 current superseded；坏签名/future/regression/同序不同内容/水位后缺 current/未知 IO 保持 pending 和 remote 不变。
- 坏 immutable bytes、不安全本地文件、权限/普通 IO 不退休；rename 前失败不退休，rename 后目录 fsync 失败读盘校准、不丢原 wire；重启重复恢复幂等。
- 旧 State 缺字段可打开；篡改终态绑定/签名/时间/水位拒绝；普通保存、重启、备份恢复保留证据；第二终态槽冲突不覆盖。
- 全局安全暂停清空全部 campaigns，可在旧素材不可补齐后发布更高 revision；保留 A01 全部现有断言。

只使用固定 Rust 1.96.0 工具路径，CARGO_HOME/RUSTUP_HOME/RUSTUP_TOOLCHAIN 固定，CARGO_NET_OFFLINE=true，cargo --offline --locked。仅 pure publisher 过滤器与 memory/tmp 数据。不会 full desktop/e03 updater；若 Tauri 构建资源校验阻断，仅复用既有 R11 environment-only runner，不改 source config。

## 5. 完成与未运行边界

交付源码差异、RED/GREEN 原始日志、精确命令、实际测试数与场景数、源 hash 和未覆盖项。源码写入和验证需等待“文件释放”。真实对象存储、网络/DNS、真实 HOME/账号、Keychain/IPC、GUI/服务以及命令/UI接线均 NOT_RUN。
