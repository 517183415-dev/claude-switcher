# 原生 Keychain：入口已建立，成功恢复仍待验

状态：`WAIT_NATIVE_KEYCHAIN_NO_DEFAULT_IN_RESTRICTED_PROCESS`。没有把失败尝试记为通过。

新增 Personal/macOS 忽略测试，实际调用正式 `KeychainVault` 和 `Publisher`。计划验证独占创建、两个线程竞争同一条目、加密备份、错误口令、恢复后旧代次与旧密钥保留、同进程重新打开。上述正向检查本轮未完成：首次创建即返回 `SecretUnavailable`。

只读 Security.framework 诊断返回 `SecKeychainCopyDefault = -25307`（无默认 Keychain）。隔离 HOME 和真实 HOME 配合相同资料读取禁令，均得到该结果。它证明当前受限进程没有可用默认 Keychain；不能据此判定系统用户没有 Keychain，也不能断言已发现产品缺陷。没有放开真实 Library 的读取，没有更改默认 Keychain、搜索列表或锁屏。

已验证：测试二进制编译、Personal 主程序和测试 Clippy、两个修改文件格式检查。Clippy 保留三条既有迁移风格警告。默认执行只忽略此原生测试；缺少显式启用参数时，测试在创建目录和调用 Keychain 前拒绝。两种安全保护的日志分别保留。测试失败后执行清理，拥有条目清单为空；本轮成功创建的条目数为 0。

`run.py build ...` 构建并记录二进制，`run.py native` 只运行该忽略测试。native 要求全新 synthetic-state 目录；不要覆盖已有证据。`probe.py` 可重现只读诊断。构建使用原 account-free 策略；native 策略仅解除 securityd mach-lookup 禁令，继续拒绝真实资料、外网和系统操作。测试进程禁用 Keychain 授权弹窗。

[汇总回执](receipt.json)绑定源文件和依赖清单；[原生失败](native.log)、[保护拒绝](opt-in-guard.log)、[默认忽略](default-skip.log)单独保留。build.log、二进制和合成目录留在本机，不提交。未重复原已有效的 Public、前端、236/35 主程序全量或系统测试。UI 重写仍最后执行，沿用当前布局和非蓝紫色配色；注册、部署、付款暂停。
