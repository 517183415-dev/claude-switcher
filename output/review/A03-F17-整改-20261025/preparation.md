# A03 核心预备设计（2026-10-26）

状态：两个新模块已写，尚未接入 mod.rs。没有运行 Rust、测试、格式化、编译或产品命令。不能标 GREEN 或已完成 F17。A02 writer/root 占用的现有文件未修改。后续接入等待 root 明确释放。

## 数据与最小分组

核心接入组为4文件：新 cleanup.rs、新 cleanup_tests.rs、现有 mod.rs、local_store.rs。ObjectStore/protocol.rs 不变。命令、DTO、UI、权限由 root 后续独立组处理。

State 后续新增 serde(default) cleanup: Option<CleanupBatch>。Batch 只持有schema/id/generation/checkpoint_hash/phase/候选ImageAsset（<=128）与第二确认的有界文件身份，不内嵌旧State。history上限256、assets上限128均不变。默认N=128，N包含两current对应的现有固定历史；再从最新追加位置补足N。不存在对应已验证历史时只保护current引用，不补造HistoryRecord；同release/channel已有记录而payload不同则拒绝。

固定文件：当前generation/cleanup-checkpoint.json，单份旧State，<=STATE_LIMIT（16MiB）。先写checkpoint，再一次persist裁history/assets、可选显式释放last_resolution、写Batch。checkpoint hash绑定到Batch。旧State内cleanup必须为空，防止批次链与快照递归增长。State.cleanup为空时残留checkpoint不是活动批次；只有下次显式整理在预览绑定其完整hash/安全身份后才可原子覆盖。第一次确认明确替换旧撤销证据。未知文件、旧generation、备份永不顺带删除。

## API与认证/预览

拟四个明确动作：cleanup_preview(options)、cleanup_commit(preview_digest)、gc_preview(batch_id)、gc_commit(gc_digest)。options默认retain=128、release_resolution=false。无自动清理、无额外abandon命令。

FixedCurrents只提供read_currents返回preview/production固定二元素数组。生产adapter必须在同一个LocalObjectStore publication生命周期锁下读取，且持锁贯穿单次preview/commit；不能同时open两个LocalObjectStore，也不能从任意URL读。Publisher原生命周期锁同时保持。

预备retention读取已验证State、认证两current并检查ETag、水位、未来时间、退序及同序冲突。过期/disabled current仍保留引用。缺current且已有水位拒绝。草稿、pending、保留history、两current、未明确释放的A02 evidence均计入保护；任何受检查manifest的素材须与登记ImageAsset完全一致。pending允许只读preview，但commit必须拒绝。未释放checkpoint保护文件至第二确认，第二确认明确放弃本批完整撤销能力。

Preview digest绑定generation、完整State摘要、选项、两current wire hash+ETag及索引候选清单。接入wrapper另绑定固定旧checkpoint hash/身份、active batch状态。改变State/generation/current/ETag/options/候选或旧checkpoint后，旧preview拒绝。N=256且全部保留明确显示无历史容量释放。

第一次整理不打开/解码全部素材，也不调用export_backup。坏素材或64MiB备份JSON膨胀不能堵住索引整理出口。候选bytes取已验证登记元数据，标为预计占用；实际GC清单在下一步读取文件后单独显示。

## 两次确认与GC崩溃边界

```text
只读索引预览 → 第一次确认 → checkpoint原子写 → State一次原子提交
                                                  ↓
准确GC预览(四类) → 第二次确认 → persist gc_started与确认身份
                                                  ↓
逐项重查引用/身份并只删eligible → fsync assets → persist清cleanup
```

GC四类为eligible、already_missing、blocked、referenced。第二确认明确说明：只删除eligible，缺失项视为已不存在，blocked/referenced保留，完成本批后残留文件以后不自动删除。损坏、链接、异常权限/owner/nlink、hash/identity不符均不能进入eligible。每次仅固定assets目录内本批登记sha256文件，使用O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC打开FD，检查REG/owner/0600/nlink1/dev/ino/size/hash。禁止递归、任意输入路径、按mtime清理。

先persist gc_started+准确确认清单，再发生任何删除。每次删除前重新计算新引用并重查文件身份；新引用项跳过并报告。意外IO/中断保留批次，重启重新预览；已经不存在的本批候选归already_missing。删除后的assets目录fsync完成，才persist清cleanup。rename已生效但目录fsync报错时沿用A02 persist的磁盘读取校准内存，仍报告耐久性失败，不回写旧State。

预先分类blocked不会永久锁住批次：用户第二次确认已同意保留它们，本次eligible处理完成后即可清cleanup；固定checkpoint留下。后续第一预览必须明确绑定并确认覆盖旧checkpoint，之前未删除文件作为未知残留，不能进入新批自动回收。不存在无限日志，不承诺全磁盘有界。意外未预览到的IO异常则不能当作预先blocked直接结束，必须重预览。

## 与备份和A02的边界

cleanup是本机瞬态引用，不能随backup迁移源generation/inode。接入时export_backup仅在snapshot的State克隆上清cleanup；已解密认证的导入State清cleanup，然后完整执行既有验证。当前目标Publisher有active cleanup时，restore_backup先拒绝，说明先完成第二次GC确认；即使blocked均保留，也能明确结束批次。KDF、密文格式、素材、水位和A02 last_resolution逻辑不改。

release_resolution只在第一次preview/confirm显式选择才执行，默认false。释放后序号水位不降；完整旧证据仍在checkpoint中，直至第二次GC确认放弃撤销。普通save/publish通过State克隆保存活动batch，不可意外覆盖该字段。

## 预备代码与后续验证

cleanup.rs当前仅有有界数据结构、FixedCurrents trait、纯retention算法与只读inspect_candidate。没有persist/删除/GC执行路径。cleanup_tests.rs有5个未运行方法，覆盖满256/128、默认128固定两current、非法N、无history current/过期认证、水位/签名/preview摘要、pending预览及文件身份只读检查。不能把这些未接入方法算作验证。

释放文件后先冻结A02/A01基线，再接入并取得真实行为RED；缺函数/编译/夹具失败单独记录，不计RED。随后补测试矩阵：draft/pending/history/current/A02证据各自独占引用、缺未登记引用、stale preview、每个checkpoint/rename/fsync边界、GC分类/重引用/新inode、部分GC重启、持久gc_started和清batch后fsync校准、活动batch跨save/publish、active cleanup export→freshrestore及active-target restore拒绝。全部tmp/MemoryVault/合成signed current，不碰真实Keychain/系统/网络。

Rust仍只用直接1.96.0-aarch64-apple-darwin工具链、固定CARGO_HOME/RUSTUP_HOME、offline/locked、合成HOME与A01 TAURI_CONFIG。此阶段未运行任何Rust命令。
