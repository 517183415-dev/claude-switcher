# R10 / F13 最小删减计划

目标：移除用户 guardian 直接执行 systemsetup 的重复时区 writer。

1. 先运行现有 fake driver 与安装健康测试，锁定保留行为。限定选择器，排除真实 native observer 测试。
2. 仅删除 guardian::run 的 timezone spawn/abort、对应用户模块和 lib.rs 私有注册。共三个源文件。
3. 静态核验生产 writer 已无引用。guardian 除包装函数的预期删除外保持字节一致。root watchdog、helper 授权时区、Public 调度、事件、健康及停机逻辑保留。
4. 重跑定向隔离测试，编译生产 guardian 分支，执行 clippy、fmt、diff 检查及独立审查。

保留行为由既有 fake driver 测试证明。模块删除由静态引用、精确 diff 和编译证明。两类证据分开，不把 fake driver 通过当作生产包装函数执行证据。不为纯删除新增镜像实现的源码断言单测。

本地测试仅使用合成 HOME、临时文件和 fake hooks。只用已安装直接 Rust 工具链，cargo offline/locked。不运行产品、真实系统事件、XPC、服务、网络、GUI 或真实账号。不安装依赖，不使用 C2C。
