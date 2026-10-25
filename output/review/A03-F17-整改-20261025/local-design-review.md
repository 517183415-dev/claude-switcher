# A03 GC设计审查与最小修复合同

初轮结论：GC执行前存在P1设计阻断。inspect_candidate仅返回FileStamp，stat再unlink不能证明删除的仍是同一对象。不得把第二次stat称为原子条件删除。此时删除尚未实现，没有执行漏洞证据。

修复合同：本产品协作进程持Publisher生命周期锁和同一publication锁。持可信assets/quarantine目录FD。先持久化gc_started和准确确认身份、内部batch/hash槽名。再以rename EXCL移动到私有隔离目录。移动后重验dev/inode/owner/mode/nlink/size/fullhash，匹配才删除隔离槽；原hash路径的新文件不删除。发现替换保留隔离文件并报告blocked。

重启只按本批精确名单核查原路径和隔离槽。新增引用保留。意外IO保留活动批次。rename后同步两个目录；完成后State清batch若rename已发生但fsync失败，读盘校准而不回写旧State。

在上述协作锁威胁模型内，设计P1可以关闭；实施和故障测试仍需另审。本合同不声称防御可修改同UID私有目录或进程的恶意攻击者。现有owner/0700/生命周期锁模型不提供该保证。

必要回归：确认persist前后、rename前后、隔离验证前、unlink后、两目录fsync、清batch的State rename/fsync；内容/身份替换、重引用、部分回收重启、原路径与隔离槽同时存在。

其他合同：默认128且固定current历史计入N；过期current仍保护；认证水位随索引原子保存；单checkpoint、两次确认、blocked/referenced可结束但留文件；下一批显式替换旧checkpoint；backup剥离瞬态cleanup且active目标拒绝恢复。

审查者：本地只读 agent /root/audit_ads。未写源或运行测试。本记录为设计审查，不是实现验收。
