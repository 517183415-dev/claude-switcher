# 普通发行后端 IPC 隔离测试

本测试使用普通 Rust 入口、处理函数和实际两版权限配置。构建时只替换 frontendDist；不增加产品命令或 Rust fixture feature。当前测试使用 Apple Silicon macOS、GUI 会话及系统已授权的界面读取能力。

在仓库根目录执行，输出目录必须是 output 下尚不存在的目录：

```sh
python3 -B apps/desktop/tests/native-ipc-policy/run.py public output/ipc-policy-public --toolchain-bin /Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin
python3 -B apps/desktop/tests/native-ipc-policy/run.py personal output/ipc-policy-personal --toolchain-bin /Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin
```

先播种独立 HOME，关闭全部广告。沿用 account-free.sb，阻断外网、真实资料及危险系统命令。仅调用安全本机命令；需参数的选定命令故意缺参，验证进入业务前的拒绝。Public 对运营命令的测试要求明确 ACL 拒绝，不能用 Command not found 代替。

宿主读取指定 PID 的页面文本，校验完整命令顺序、结果类别、版本和设置，再独立核对磁盘设置/导出。只停止本次进程组。合成目录中间层全部为 700，配置文件为 600。工具不会注册服务、部署或付款。

`--reuse-build <旧输出目录>` 只在测试计划、脚本和产品输入不变时复用编译产物；不能跨版本复用。边界测试还必须重新绑定原构建的同一个本机端口；端口被占用时失败，不能悄悄切换端口。测试 App 使用本机临时签名，不代表正式签名验收。远程及其他窗口动态测试截至本轮仍未取得有效回执，不能计为通过。

定向回归：

```sh
python3 -B -m unittest discover -s apps/desktop/tests/native-ipc-policy -p 'test_*.py' -v
node --test apps/desktop/tests/native-ipc-policy/probe.test.mjs
```

不能把本测试当作原产品前端、完整动态矩阵、Keychain、账号登录、远端发布或系统托管 WebKit 回收的验收。


## 远程来源与其他窗口

追加 `--boundary-matrix`，普通后端会创建两个测试窗口：主窗口加载随机端口的本机 HTTP 页面，另一个窗口加载内嵌页面。权限配置保持原样。Public 对 23 个命令、Personal 对 48 个命令逐项要求 ACL 拒绝。回执必须包含预期 URL、版本、上下文、真实桥和完整命令序列。桥缺失、参数错误、命令不存在均不能代替 ACL 拒绝。遇到意外成功或其他失败类别，脚本立即停止后续调用。

本机服务器只绑定 127.0.0.1，只提供内存中的测试 HTML 和脚本。上下文退出时关闭监听器，包括异常路径。回执读取失败仍保留服务器请求记录、最后一次窗口读取结果及进程停止结果。测试客户端明确绕开继承的代理，并用失效代理环境验证此约束。

当前 Public 已构建。两次窗口回执尝试未通过；系统随后确认 Mac 处于锁屏状态。解锁前不继续窗口尝试，也不把无回执计为拒绝。下次可复用已有 Public 构建；前提是原端口可用、脚本及产品输入未变。Personal 动态构建暂缓，避免无效消耗。
