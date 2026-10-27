# 普通 App 前置隔离探针

本探针不启动普通 App、Claude 或 Keychain。它复用 `native-ipc-business/support.py` 生成的运行沙箱。

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s apps/desktop/tests/app-isolation-probe -v
PYTHONDONTWRITEBYTECODE=1 python3 apps/desktop/tests/app-isolation-probe/runner.py --output output/no-account-20261027/app-isolation-probe
```

父进程和其直接 exec 子进程使用合成 HOME 做文件读写。两者只尝试打开实际 HOME 的 `Library` 目录；即使意外获准，也立即关闭句柄，不读取目录内容。回环地址仅测试 `bind`，不会监听或连接。非本地网络只做 `network-outbound` 沙箱策略检查，不进行连接或发送。

当前 SDK 未提供可核实的服务名过滤参数。探针不尝试 `securityd` Mach lookup，且将其标为 `NOT_PROVEN`。单独的 `lookup=denied` 字符串不能证明查询方法和反向控制有效。当前 runner 不产生通过状态；其他直接进程检查通过时，它返回非零并写入 `PARTIAL_SECURITYD_NOT_PROVEN` 回执。超时仅终止自己创建的进程组。

子进程回收不代表系统代管 WebKit 进程已隔离或清理。普通 App 启动、真实 Keychain、非本地连接和系统代管进程均未验证。不得依据此探针启动普通 App。
