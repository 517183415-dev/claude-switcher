# R03 prepared 对象边界

结论：保留已接受的 R02 生产代码。R03 的 prepared 冲突用例检查已发布、尚未持久化为 restored 的活跃目标。

1. 当前 `check_uninstall_recovery` 对活跃目标核对完整 `uninstall_prepared` 指纹。修改 TARGET，即使新值合法，仍应拒绝。
2. 尚未发布的 `stage/restore-*` 残片不参与恢复来源选择。恢复从已验证备份创建新 UUID 候选，再持久化其完整身份。
3. 初始测试把“prepared”解释为所有未发布残片，并要求拒绝。因此该断言失败。此解释会新增持续验证废弃候选的义务，超出 R02 合同。
4. 独立只读 reviewer 建议将故障点移至实际发布后、restored 保存前。该用例已实测两次拒绝、journal 字节不变、fchown 为零。
5. 未发布残片改为正向回归：两次恢复可信纽约字节；篡改为洛杉矶的残片保持原 inode 和字节，未被收编或改写。

证据：`python-green-first.log` 保留初始失败；`python-published-candidate-green.log` 记录活跃候选拒绝；`python-unpublished-candidate-diagnostic.log` 记录未发布残片行为。

此解释提交 ChatGPT 独立复核。不得声称所有私有 prepared 残片变化均会拒绝。未运行安装、真实账号、系统服务或系统设置。
