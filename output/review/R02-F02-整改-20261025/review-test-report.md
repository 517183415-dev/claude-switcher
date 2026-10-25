# R02/F02 隔离回归结果

- 完整模块：127 项通过，0 失败，33.217 秒。日志：`review-full-green.log`。
- F02：34 个测试方法。真实 Personal/Public 安装→卸载流程覆盖 11 文件、4 服务。
- 新增审查回归：durable 恢复后的 inode/GID/删除漂移；check→open 与 fchown→snapshot 窗口；备份完成后、首个副作用前的 11 备份损坏；注册服务必要文件缺失；4 根文件私有恢复副本中断；两份 >2MiB 二进制。
- 有效 RED：`review-red.log` 的 6 方法中，备份损坏测试有 11 个失败子例。首次根因是 backup checker 位于 durable pending=services 保存之前。父会话修复后全绿。
- 第二轮窗口及大二进制测试首次执行时生产修复已落地，所以仅报告 GREEN。`review-import-error.log` 是已纠正的测试拼接错误，不作为行为 RED。
- 测试文件 AST 与 `git diff --check` 通过。
- 测试文件 SHA256：`da4d557b1891ae6263681d9f9e03b2ca6b7bc0f3575ea2c4d327533de5837625`。

所有新增测试使用临时 HOME/根目录与虚拟 UID/GID、签名、状态化 launchctl。未知命令失败。未执行生成的守护脚本，未安装产品，未访问真实账号、系统服务或网络。原有临时 CLI shim/stub 测试保留。真实 launchd、真实 root 元数据和掉电持久性未验证。最终稳定源验收由父会话负责。

## Iteration 2

- 增加9方法、65场景；累计43个F02方法，136个模块方法。
- 三个有效RED日志共53失败子例：`iteration2-red.log`、`iteration2-disabled-red.log`、`iteration2-old-journal-first.log`。
- 生产修复后9方法/65场景通过：`iteration2-green-first.log`。原有34方法与新增9方法合计43通过：`iteration2-f02-first.log`，42.608秒。
- 原有22次物理删除前后中断与74次事务journal-save中断仍通过。新移入holding机制由既有rename_exclusive物理hook直接覆盖。
- holding/component是journal记录的同卷原对象备份。holding不属于活动安装路径。异常对象始终保留；不会因清理而删除。
- Python3.9 pathlib缓存OS accessor会绕过后置os.stat/lstat patch。fixture追加同源Path.stat/lstat适配，保持真实mode/dev/ino与同一虚拟UID/GID。系统3.9真实Personal/Public roundtrip已通过。
- 测试文件已冻结。父会话负责最终稳定hash括号下43/136验收。
