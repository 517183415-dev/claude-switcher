# 普通发行后端 IPC 隔离测试

本测试使用普通 Rust 入口、处理函数和实际两版权限配置。构建时只替换 frontendDist；不增加产品命令或 Rust fixture feature。当前测试使用 Apple Silicon macOS、GUI 会话及系统已授权的界面读取能力。

在仓库根目录执行，输出目录必须是 output 下尚不存在的目录：

```sh
python3 -B apps/desktop/tests/native-ipc-policy/run.py public output/ipc-policy-public --toolchain-bin /Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin
python3 -B apps/desktop/tests/native-ipc-policy/run.py personal output/ipc-policy-personal --toolchain-bin /Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin
```

先播种独立 HOME，关闭全部广告。沿用 account-free.sb，阻断外网、真实资料及危险系统命令。仅调用安全本机命令；需参数的选定命令故意缺参，验证进入业务前的拒绝。Public 对运营命令的测试要求明确 ACL 拒绝，不能用 Command not found 代替。

宿主读取指定 PID 的页面文本，校验完整命令顺序、结果类别、版本和设置，再独立核对磁盘设置/导出。只停止本次进程组。合成目录中间层全部为 700，配置文件为 600。工具不会注册服务、部署或付款。

`--reuse-build <旧输出目录>` 只在测试计划、脚本和产品输入不变时复用编译产物；不能跨版本复用。测试 App 使用本机临时签名，不代表正式签名验收。当前远程分支只受单元校验覆盖，不属于动态通过证据。

定向回归：

```sh
python3 -B -m unittest discover -s apps/desktop/tests/native-ipc-policy -p 'test_*.py' -v
node --test apps/desktop/tests/native-ipc-policy/probe.test.mjs
```

不能把本测试当作原产品前端、完整动态矩阵、Keychain、账号登录、远端发布或系统托管 WebKit 回收的验收。
