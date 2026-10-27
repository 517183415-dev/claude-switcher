# 普通发行 IPC 权限策略测试

使用已安装 Tauri 的真实权限解析器，读取构建生成的 manifest、capability 和实际两版配置。测试不创建 WebView、DesktopState 或原生服务，也不调用业务命令处理函数。

Personal 的 48 个命令、Public 的 23 个命令与普通注册表完全一致。全部授权限定 Local 来源与 main 窗口；不另授予 webview 范围。检查其他窗口、代表性远程 URL、夹具命令、未知及未授权插件命令拒绝。Public 另拒绝 25 个 Personal 运营命令。

2 个不同测试名在两版执行，共 4 次通过。两版 all-target Clippy exit 0；保留既有 migration 风格及 ts-rs 属性解析警告。格式首检仅导入顺序失败，rustfmt 修正后通过；函数体未变，复用有效测试。普通测试模块在 ipc-business 夹具中排除的有效检查亦复用。

[回执](receipt.json)记录最终源码摘要和本地日志摘要。[检查记录](verification.json)记录命令、退出码、执行时输入及格式修正前后摘要；[格式修正前](verification-before-format.json)保留失败；[前一轮记录](verification-before-scope-assertion.json)保留复用的夹具排除检查。原始 .log 按仓库策略不提交，留在本机。测试在独立 HOME 和既有 Seatbelt 策略内执行，不证明真实 Keychain 隔离。

复现：在仓库根目录，分别以 `personal`、`public` 执行：

```sh
cargo test --offline --locked -p switcher-desktop --no-default-features --features personal --bin switcher-desktop ipc_policy_tests -- --nocapture
cargo test --offline --locked -p switcher-desktop --no-default-features --features public --bin switcher-desktop ipc_policy_tests -- --nocapture
```

完整隔离环境、固定工具链路径及策略摘要见检查记录。普通 App 的完整动态 IPC、真实 Keychain/R2、远程传播仍未验证。注册、部署、付款暂停。后置 SwiftUI 重写保留当前布局，只参考玻璃视觉效果。

源码摘要统一存放在 `source-inputs.json`。各检查记录的 `overrides` 覆盖当轮差异，`removed` 表示当轮不存在的输入，避免重复保存相同清单。
