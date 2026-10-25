# R02 / F02 第二轮整改

状态：本地隔离验证、独立代码复核及 ChatGPT 第二轮接受均完成。单元状态 PASS / ACCEPTED，限源码与隔离本地层。详见 chatgpt-review-iteration2.txt。

1. 删除先将目标独占移动到同卷 holding/component，再验证实际移动对象的内容、身份和元数据。CLI 操作使用账号所有者身份。journal 记录派生 holding 目录及目录身份。异常对象保留；恢复及卸载完成检查均拒绝该冲突。holding 是事务原对象备份，不是活动安装路径；本轮不自动删除这些备份。
2. 恢复先验证准备文件，保存完整 uninstall_prepared 指纹，再独占发布。CLI 在退出所有者身份后保存 root journal。恢复仅接受已记录的候选身份；旧 journal 只有 intent 时拒绝不明确的 live 文件。最终所有权调整仍检查打开文件的身份。
3. 进入或续接卸载回滚的 restored 终态前，重新检查 11 个文件、4 个服务注册、disabled override 与原 legacy 状态。冲突保留 journal、barrier 与外部对象。其他安装/替换终态和已无 journal 的历史 restored 结果不扩展为持续监控。

## 验证

- 修复前：新增 9 方法分三次运行；53 个失败子例。三个原始 RED 日志分别保留。
- 修复后：43 个 F02 方法通过；136 个整模块方法通过，其中包含这 43 个 F02 方法。
- 新增方法包含 65 个故障或正向场景。原有 74 个事务 journal-save 中断场景、22 个实际删除前后场景继续通过。它们与恢复发布/终态场景分别计数。
- 两次最终命令各自比较运行前后源码哈希；均稳定。git diff --check 通过，F01 三文件哈希保持已接受值。见 verification.json。
- 系统 Python 3.9 对新增 65 场景通过。测试 fixture 补齐 pathlib 缓存 accessor 的模拟，保留实际 mode/dev/ino/bytes/nlink。
- 安装、真实账号、实际系统服务、原生 UI、长期运行验收未执行。

## 历史快照说明

iteration1-verification.json 记录首轮 34 / 127 结果。iteration1-local_install.py 是首轮生产源码快照。iteration1-test_local_install.py 在第二轮新增测试期间复制，不能用该文件名主张它对应首轮 127 方法时的精确测试源码；首轮测试源码哈希以首轮 receipt 为准。
