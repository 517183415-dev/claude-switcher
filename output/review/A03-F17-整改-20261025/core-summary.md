# A03/F17 core 实施记录

## 写入范围与合同

只修改 `apps/desktop/src-tauri/src/publisher/{mod.rs,local_store.rs,cleanup.rs,cleanup_tests.rs}`。没有修改 A01/A02 `tests.rs`、`backup.rs`、protocol、命令、UI、DTO 或权限文件。后者由父会话独立接线。

- history 上限 256 / assets 上限 128 不变。默认保留最近 128 条，已认证两 current 匹配的既有历史包含在 N 内。没有匹配历史只保护引用，不虚构 HistoryRecord。
- draft、pending、保留历史、两 current、A02 resolution 引用按同一锁内认证保护。过期 current 仍保护；缺 current 且有水位、未来、退序、同序冲突、引用缺登记均拒绝。pending 可以预览，不能确认执行。
- 第一次确认：旧 State 写固定 `cleanup-checkpoint.json`，然后一次 state persist 裁剪索引并保存 batch、认证水位。没有全量备份或完整素材读取前置。显式 release_resolution 默认 false；清单绑定该选择。
- 第二次确认：eligible / already_missing / blocked / referenced 四类。只回收 eligible。完成本批可保留 blocked/referenced 文件并清 batch；固定 checkpoint 保留。下一次首次预览绑定旧 checkpoint 身份/hash，并要求确认替换旧证据。
- GC 只操作本 generation/assets 的名单。先持久化确认身份与精确 slot，随后在可信目录 FD 间 EXCL rename 到 `cleanup-gc/<batch>-<hash>`。重新核对 metadata/full hash 后只 unlink 隔离名，永不 unlink 新建的原 hash 文件。未知文件、旧 generation、普通备份均不扫描、不删。

```
index preview -> confirmation -> checkpoint + indexed batch
                                      |
GC preview -> confirmation -> gc_started + identity
                                      |
assets/hash -- EXCL rename --> cleanup-gc/batch-hash
                                      |
                         FD identity/hash -> unlink slot
                                      |
                     sync both dirs -> clear batch
```

## 原子点与恢复边界

源+隔离四种状态均按本批名单处理：源有/槽无重新核验；源无/槽有验证原确认身份；两者有只处理已确认隔离槽并保留源；两者无为 already_missing。损坏或身份变化保留为 blocked。意外 IO 保留 batch，不把未知结果标作可结束。所有已存在引用在删除前再认证。

终结 batch 前无条件检查并同步 assets 与 quarantine 两目录，包括全 already_missing 的重试。State rename 后报错使用既有 persist 的磁盘/内存一致性校准。

这是同 UID 协作产品进程遵守锁的合同。未声称抵御恶意同 UID 修改私有隔离名。故障注入是合成异常与 drop/reopen；未执行 SIGKILL、断电或物理耐久试验。

## 容量与备份

`reserve_gc_state` 根据真实序列化 State/batch 大小，以及最大整数、最长 class 与完整固定 slot/hash 的 worst-case batch，计算 GC 身份所需上界。首次确认在 checkpoint 写入前验证；普通 persist 在 Indexed/GcStarted 两阶段保留此空间。读取/open 不加更严门槛。

近 16 MiB 压力 fixture 使用既有 Sequence hash 字段填充，现有 validate_state 接受该合成 State。它是编码容量测试，不代表真实素材或真实发布自然达到此大小；无新增大图片、KDF 或真实素材。

导出 snapshot 清 cleanup；认证解密后的导入 snapshot 清 cleanup 再走既有完整验证。目标有 active cleanup 时 restore 拒绝，要求先完成第二次确认。KDF、密文格式、素材、水位、A02 resolution 保留逻辑没有改变。

## 行为 RED 与其他失败

- `index-red.*`：真实容量释放断言失败；旧 seam 确认后仍不能导入第 129 个素材。exit 101。
- `gc-red.*`：真实物理回收断言失败；旧 no-delete seam 没有删除确认候选。exit 101。
- `finish-sync-red.*`：真实重启后全 missing 的终结路径跳过必要目录同步。exit 101。
- `capacity-red.*`：首次确认允许无后续 GC 身份编码余量的 batch。exit 101。
- `core-matrix-green.*` 首轮 13/15：测试 fixture 父目录未建、清 pending 后候选计数应为 4 非 3。不是行为 RED。
- `core-matrix-green2.*`：cfg 属性插入位置错误导致编译失败。不是行为 RED。
- `core-matrix-green3.*` 16/17：LocalObjectStore fixture 缺 application_root/v2。不是行为 RED。`local-store-fixture-green.*` 已修复通过。
- `core-clippy.*` 首轮三个 collapsible_if，已按建议改写，无 allow 绕过。

## 验证环境与范围

固定 direct cargo/rustc/rustdoc/rustfmt 1.96.0；`--offline --locked`；固定既有 Cargo/Rustup cache；合成 HOME/TMPDIR；validation-only TAURI_CONFIG resources=[] 与 R07 已有 dist。没有调用 rustup 代理、下载、安装、网络/回环服务、系统服务、真实账号、Keychain、GUI 或产品进程。

每次 `run_check.py` 保存完整 stdout/stderr、准确 argv/exit/elapsed、前后源码 hash 和 source_frozen。只跑 cleanup/f16 方法，没有全跑 desktop/e03。真实 IPC/WebView、系统 Keychain、生产数据、物理断电均 NOT_RUN。A01 64MiB 新 State 回归由父会话统一执行。

最终 `core-final-green.*` 为 19/19，exit 0，29.48 秒；`core-fmt-check.*` exit 0。最终 Clippy 由父会话统一执行，不能用首轮失败宣称通过。完整 hash 见 `core-frozen-hashes.json`，源码副本为 `final-*.rs`。

## 容量上界推导

1. 所有 batch candidates 最多 128，经过既有 ImageAsset 验证；每个 slot 必须是固定 32 字符 batch id、一个连字符及 64 字符 SHA256。
2. 对每个候选保留完整 asset 和完整 FileStamp。dev/ino 使用 u64::MAX，uid/mode 使用 u32::MAX，nlink 固定 1，bytes/hash 使用候选精确值。所有实际身份数字的 JSON 长度均不会超过此表示。
3. phase 使用 `gc_started`；class 使用最长枚举序列化值 `already_missing`；quarantined 使用比 true 多一个字节的 false；identity 使用对象而非较短的 null。
4. 上界为 `encoded(State).len - encoded(current batch).len + encoded(maximum batch).len`。Rust checked_sub/checked_add 拒绝溢出。没有双份完整 State 内嵌，也没有凭经验固定留白。
5. 最大 128 候选 fixture 在 `STATE_LIMIT - 4096` 时验证首次确认与 Indexed 普通写入拒绝；GcStarted 的 `STATE_LIMIT - 1` fixture 验证再次预览身份的空间仍受保护。原始 State 及 checkpoint 在拒绝前保持不变。默认 128 保留清满历史及释放满素材索引由独立方法验证。

## 场景回执

- 19 个独立 f17 测试方法。原子边界矩阵分别覆盖 checkpoint/index 4 个注入点、GC 10 个注入点、state rename 写前/写后 × index/start/finish 6 个组合、终结 assets/quarantine sync 2 个注入点。它们是同进程合成异常 + drop/reopen，不是电源耐久验证。
- 四类 GC、被替换源/隔离槽、重建原 hash、源与槽同时存在、意外 IO、已有索引重新引用、未知文件保留均有实际临时文件断言。
- 本地 adapter 使用一个真实临时 LocalObjectStore 锁读取两个 current；同根第二个 store 打开被拒绝。普通 draft save 与本地 publish 保留 cleanup batch。
- active export→fresh restore、导入清瞬态字段、active target restore 拒绝使用 MemoryVault。没有系统 Keychain。

## 核心锚点

- `cleanup.rs:508`：GC 状态编码余量。
- `cleanup.rs:701`：第一次确认、checkpoint 与原子索引提交。
- `cleanup.rs:784`：四类预览与身份绑定。
- `cleanup.rs:900`：隔离移动、复核、删除、最终目录同步与批次终结。
- `mod.rs:353`：所有普通 persist 的余量门槛与磁盘校准。
- `mod.rs:1199` / `mod.rs:1226`：备份瞬态边界。
- `local_store.rs:203`：同一 publication 生命周期锁下的两 current。


## 冻结回执

`f16-final-green.*`：18/18，exit 0，18.46 秒。与最终 19 个 f17 合计 37 个独立方法通过。验证期间 6 个记录文件 hash 不变。A01/A02 原 tests.rs 与 backup.rs 均保持阶段基线 hash。

独立审查者 audit_ads 已复核 live hash、两个新增真实 RED、19/19 GREEN、容量上界与终结同步，确认两项修复可关闭。父会话接管 A01 f15_ 回归、Personal Clippy 和 Public check；子任务不重复运行。四核心文件已停止写入。
