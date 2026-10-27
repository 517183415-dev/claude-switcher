# 解锁后的原生 IPC 边界验证

产品源码基线：`a7f61dce`。仅修改测试运行器及其回归测试，未改产品 Rust、前端、ACL 或系统设置。

| 版本 | 远程来源主窗口 | 内嵌页面其他窗口 | 有效证据 |
| --- | --- | --- | --- |
| Public | 23 条 ACL 拒绝 | 23 条 ACL 拒绝 | [保存回执的修正校验](../ordinary-ipc-runtime/public-unlocked-r2/validated-receipt.json) |
| Personal | 48 条 ACL 拒绝 | 48 条 ACL 拒绝 | [原生运行回执](../ordinary-ipc-runtime/personal-unlocked-r1/receipt.json) |

共 142 条原生 IPC 响应。它们是完整命令序列的响应数，不是 142 个独立测试名。每个上下文均检查版本、实际 URL、真实 IPC 桥、顺序和明确 `not allowed` 拒绝；缺桥、缺参或命令不存在均不能代替拒绝。

## 两个测试工具缺陷

1. 旧 Seatbelt 策略允许本机 TCP，但拒绝 WebKit 所需的 `/private/var/run/mDNSResponder` Unix 套接字。原生最小 WebKit 诊断得到 `NSPOSIXErrorDomain Code=1`；仅允许该套接字后，原始持久化 WebKit 即成功加载。生产 ACL 和原隔离文件未修改。新增运行器回归实测解析套接字可连接，其他合成本机套接字、直接外网 IP 连接和真实 Library 元数据读取仍被拒绝。RED/GREEN 日志保留。
2. WKWebView 将 `tauri://localhost/` 序列化为 `tauri://localhost`，旧校验器误拒绝。校验器只接受这个根 URL 的两种等价形式，仍拒绝其他协议、主机、端口、路径、查询或片段。新增回归先失败再通过。Public r2 的原生回执已完整保存，因此只重新校验这些回执，没有再次启动有效的窗口测试。

`public-unlocked-r1` 保留解析器修复前的远程超时及其他窗口回执；其构建被 r2 原样复用。ATS 声明和去除代理环境的诊断尝试未解决问题，仅保留在本机诊断目录，不计通过，不归因为产品缺陷。原始直接 Python 本机连接成功，也不足以证明 WebKit 可连接。

## 验证和复现

- `python3 -B -m unittest discover -s apps/desktop/tests/native-ipc-policy -p 'test_*.py' -v`：14 项通过，包含两项新增回归。最后完整执行见 `../ordinary-ipc-runtime/webkit-diagnostic/final-tests.log`。
- Public：原命令为 `python3 -B apps/desktop/tests/native-ipc-policy/run.py public output/no-account-20261027/ordinary-ipc-runtime/public-unlocked-r2 --boundary-matrix --reuse-build output/no-account-20261027/ordinary-ipc-runtime/public-unlocked-r1 --toolchain-bin /Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin`。修正回执校验脚本为该目录 `validate_saved.py`，不重启 App。
- Personal：`python3 -B apps/desktop/tests/native-ipc-policy/run.py personal output/no-account-20261027/ordinary-ipc-runtime/personal-unlocked-r1 --boundary-matrix --toolchain-bin /Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin`。重新执行必须换新输出目录。
- 两版普通入口采用测试 frontendDist 与测试窗口，保留生产注册表、处理函数和实际权限；合成 HOME、广告全关闭、外网/真实资料/危险系统命令仍受阻。只停止本次进程组，本机服务器均退出。直接进程由测试 SIGTERM 停止，不能称为产品崩溃，也不证明系统托管 WebKit 子进程回收。
- 产品输入保持基线不变。回执分别绑定运行时输入与二进制；文档更新后的输入差异另列，不改写原始输入文件。Rust 完整测试和 Clippy 沿用前一阶段有效结果，本阶段只集中运行改动相关的完整 Python 集合。

## 验收边界

完成的是指定两个拒绝上下文的动态矩阵，不代表未修改的产品前端、全部成功业务、原生 Keychain、真实账号、R2/CDN、安装/系统组件或长期观察通过。SwiftUI UI 重写继续最后执行，保持当前布局与非蓝紫色配色。注册、部署和付款保持暂停。
