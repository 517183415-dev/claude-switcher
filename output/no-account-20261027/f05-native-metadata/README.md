# F-05 原生文件系统测试补足

状态：局部原生验证通过，不能据此宣称 F-05 全分支或 35 项全部验收通过。

## 当前有效范围

`crates/switcher-core/src/accounts/tests/native_metadata.rs` 调用 `AccountStore::select_legacy_checked → accounts::legacy::select`。它使用与生产相同的事务、真实 `/usr/bin/ditto` 和 Darwin 目录交换。仅注入 Claude 已退出/仍运行的观察结果，不控制真实 Claude 进程。全部资料来自临时合成目录。

两个新测试覆盖：

- A→B→A：A/B 最新嵌套二进制内容、目录/文件 mode、uid/gid、mtime 纳秒、二进制 xattr、复制后 inode 分离及归档保留；核对状态、current 和事务清理。
- Claude 仍运行：生产入口拒绝切换；active、A 归档、状态和 current 逐字节/元数据保持，不创建 B 或残留事务。

`final/snapshots.json` 保存实际 APFS 树快照。断言直接使用 Darwin getxattr 和文件系统元数据，不使用业务 fingerprint 作为预期值。没有调用真实账号、Keychain、安装器、代理配置或线上服务。

生产合同允许没有存档的 B 使用空白资料。旧测试辅助函数的 NotFound 行为不能作为生产合同。`r1/` 为旧测试专用事务的中间执行；`r2/` 是生产路径的中间执行。两者均不作为最终源码的验收证据，原始文件保留在本地。

## 验证结果与失败记录

[机器回执](receipt.json)和 `final/inputs.json` 绑定执行源码。集中核心测试首次为 441 通过、10 失败，共 451 项。10 个失败原样保留在 `final/core-tests.log`。

1. 9 个受控 TLS 测试使用 `/usr/bin/python3` 启动器。该启动器在空环境下添加 CPATH、LIBRARY_PATH、MANPATH 和 SDKROOT，触发测试已有的环境隔离断言。`final/python-environment-probe.json` 实证此行为。改用 Xcode 的实际 Python 可执行文件后，只重跑这 9 项并全部通过；`final/direct-python-probe.json` 保存对照。
2. 时钟测试需要读取 `sysctl -n kern.bootsessionuuid`。原沙箱禁止执行 sysctl。只为该项移除这一执行禁令，其余隔离规则保留，单项重跑通过。`final/retry-account-free.sb` 保存差异。未修改产品时钟逻辑或系统设置。
3. 合计 451 个不同顶层核心测试均取得通过结果；这表示首次 441 项与后续 10 项的并集，不是一条命令全绿。没有重跑已通过的 441 项。`final/retries.json` 保存精确测试名、命令、环境 PATH、退出码和日志摘要。
4. Clippy `switcher-core --all-targets -- -D warnings` 通过；新 Rust 测试格式检查通过。全部 Rust 执行均隔离 HOME，拒绝外网及真实资料访问。

## 源码忽略规则回归

`.gitignore` 原有 `accounts/` 规则也排除了 Rust 账号源码目录，导致新测试不能正常加入 Git。只对 `/crates/switcher-core/src/accounts/` 增加源码例外；根目录和生成 HOME 的账号资料继续被忽略。

`tests/test_source_tracking.py` 用 `git check-ignore --no-index` 同时检查两条源码路径和两条资料路径。`final/ignore-red.log` 保存修复前失败；`final/ignore-green.log` 保存修复后通过。没有改为全局开放 accounts 目录。

## 图谱与未覆盖范围

CodeGraph 已增量同步：235 文件、6282 节点、26222 条边，complete、pending 0。此前被忽略的账号模块现已纳入索引。状态见 `final/codegraph-status.json`。

本次没有验证真实账号登录保持、实际退出 Claude、跨卷、磁盘真实耗尽、断电持久性或所有复制失败分支。普通窗口来源权限矩阵、真实系统安装、外部发布源及长期验收仍保留等待/未验状态。注册、部署、付款继续暂停。原生 SwiftUI 玻璃 UI 重写保持最后，沿用当前布局。
