# 账号事务与更新入口的跨进程竞争

本轮新增一项跨程序回归。它分别运行 Personal 和 Public。测试使用合成账号资料、合成时钟和临时目录。

1. 桌面更新入口持有生产生命周期锁时，账号协调器的实际切换入口拒绝切换。活动资料、当前账号标记与既有更新状态不变。
2. 测试终止并回收持锁的更新测试进程。账号协调器随后完成 A→B，并保留 A 的资料。
3. 账号协调器在事务中持锁时，桌面更新的设置操作、检查任务和状态读取均报告锁占用。已有更新状态与时钟文件不变。
4. 账号事务恢复后完成 B→A。桌面更新入口随后能够保存设置。

复现入口是 [跨程序测试脚本](../../../../tests/update_account_contention.py)。运行 `python3 tests/update_account_contention.py`。该测试需要另一个包的测试程序，因此在普通测试集中标记为 ignored；此脚本会分别构建核心与桌面测试程序，再显式运行该测试。沙箱使用独立 HOME 和临时目录，阻止网络、默认 Keychain、真实账号目录及系统配置命令。

最终脚本执行通过：[日志](../../../../output/no-account-20261027/update-account-contention-20261028/runner-final.log)。两个包的测试目标严格 Clippy 通过；最后调整核心断言后，核心 Clippy 再次通过。[初次 Clippy](../../../../output/no-account-20261027/update-account-contention-20261028/clippy.log)、[最终核心 Clippy](../../../../output/no-account-20261027/update-account-contention-20261028/clippy-core-final.log)。新增 Rust 文件的格式检查通过。

最初尝试让桌面测试直接调用核心私有事务接口，编译被拒绝：[日志](../../../../output/no-account-20261027/update-account-contention-20261028/test.log)。随后改为从核心测试调用账号协调器，没有放宽产品接口。这是测试接线错误，不是已发现的产品缺陷。

测试调用的是产品账号协调器和更新运行时，停止判断及时间来自测试替身。它没有运行完整 GUI、CLI、XPC 或真实 guardian，没有停止真实 Claude，也没有测试真实断电和系统重启。原有停止路径证据和正式验收状态不变。
