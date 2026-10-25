# R08 / F07 首次 GUI 与安装收尾调用证据

本记录来自 CodeGraph 与只读代码核对。没有运行真实 GUI、原生 IPC、网络、系统或安装。

```text
App onMount → bootstrap → runtime_info → workspace_snapshot → check_environment
安装 plan → installation_settings → cli_install → finalize → component committed
环境页 修复环境 → preview_repair → execute_repair → completed=true → check_environment
```

1. App.svelte:198/210 的正常 bootstrap 调 runtime_info、refresh 和 runChecks。commands.rs:9/13/17 接入 core runtime_info、DesktopState snapshot/check。bridge.rs:84/165/194 只建状态、汇总快照及只读读取配置后检查。它不探测、保存目标或执行 fix。
2. config.rs:133 优先读取 v2，随后读取 legacy shim literal。无配置的三个目标为 None。安装器 installation_settings:1164 优先既有 v2/CLI 字面值，仅缺目标时 detect_region；探测必须非 CN 且国家与时区来源一致。cli_install:1059 将目标写入既有 shim，供 Rust 只读 fallback。
3. finalize:1981 校验组件、watchdog/bodyguard/CLI 与进程。默认 authenticated=False 直接提交组件，没有 fix 或14项环境检查。authenticated challenge/receipt 分支是安装健康身份验证，不能当成环境全绿。
4. 已有 GUI preview_repair/execute_repair 复用 RegionCoordinator。已有 CLI fix 经 NativeCli::repair → repair_cli::run_with_output → NativeRepair::prepare/apply → coordinator。首次缺目标有既有 DetectRegion 路径，不能用 Fix 伪造目标。
5. 安装 main:2131 持 locks 并调用 finalize:2146；legacy coordinator 执行也申请 LifecycleLock。因此环境后续动作若需执行，必须在组件提交后、释放安装锁后，以 owner 身份调用原入口。不能在 root finalize 持锁内调用 fix。

受保护合同：R07 请求编号只消费一次、同步父锁和 epoch；代理/国家门禁；RegionCoordinator 的生命周期锁、pending 恢复检查、配置重比对和必要授权。无自动启动 Claude。

最小边界：保留现有 GUI 和 CLI 修复器；先修歧义配置选择，再明确组件完成与环境 NOT_RUN 的交接。具体接线等待当前 R08 C2C PLAN。首个原生 GUI 全流程仍为 NOT_RUN。
