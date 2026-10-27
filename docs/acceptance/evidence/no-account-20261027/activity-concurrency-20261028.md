# 活动记录并发修复

活动记录采用读取、追加和替换文件的方式保存。此前多个操作并发执行时，缺少共同的锁。一次 Personal 合成回归中，32 条记录只保留 2 条；同时执行的读写操作共出现 17 个错误。[修复前日志](../../../../output/no-account-20261027/activity-concurrency-20261028/red-aggregate.log)。这是受控竞争下的结果，不代表实际用户的发生频率。

现在两版分别使用独立的 `activity.lock`。读取已有记录和追加记录使用同一把跨进程锁。锁最多等待两秒，超时返回可重试错误。锁文件的安全检查复用现有文件锁；空记录读取不会创建目录或锁。原有 100 条、60 KiB 限制和脱敏规则不变。

1. 两版分别完成 32 个写入线程与 4 个读取线程竞争；所有记录均保留。另有真实子进程验证锁占用及释放后的写入。
2. 不安全的软链接、硬链接锁文件被拒绝；原记录和目标文件不变。空记录读取不创建目录或锁。持锁超时不改变已有记录，释放后重试成功。
3. Personal 测试目标分别通过 36 项与 34 项，另有各 1 个忽略项；其中 25 项相同的更新测试被两个目标重复执行，不能当作独立覆盖累计。Public 定向测试通过 4 项，另有 1 个子进程夹具忽略。新增超时测试单独通过。子进程夹具已由父测试调用。
4. 核心存储相关测试 26 项通过。专用隔离运行器的 4 个阶段通过，覆盖两版活动文件写入被拒绝后，已有日志不变且已提交设置可重新读取。普通测试集中该故障测试标记为忽略，由此运行器单独执行。
5. 两个受影响包的严格 Clippy、Public 定向 Clippy 和格式检查通过。相关记录：[Personal](../../../../output/no-account-20261027/activity-concurrency-20261028/personal.log)、[Public](../../../../output/no-account-20261027/activity-concurrency-20261028/public-activity.log)、[超时重试](../../../../output/no-account-20261027/activity-concurrency-20261028/busy-lock.log)、[故障回执](../../../../output/no-account-20261027/activity-concurrency-20261028/fault-run/receipt.json)、[核心存储](../../../../output/no-account-20261027/activity-concurrency-20261028/core-storage.log)。

## 本地产品包

Personal 和 Public 已从源码提交 `4b0223ea` 的 Git 归档重新构建。两版均为 arm64、最低 macOS 13，复制后的包通过本地 ad-hoc 签名校验；没有公证。[构建回执](../../../../output/no-account-20261027/activity-concurrency-20261028/package-r2/receipt.json)。

构建使用本机依赖与缓存，并在隔离沙箱中执行。归档副本的 Swift 编译命令临时加入 `-disable-sandbox`，外层沙箱保持生效；工作区源码没有修改。首次归档管道以 141 中止，未进入构建；第二次先保存完整归档后构建成功。

测试使用合成目录，没有使用真实 Claude 账号、默认 Keychain 或远程服务。新包未进行窗口或安装验证。没有重跑工作区全部测试，旧版完整测试结果仍属于各自的源码快照。
