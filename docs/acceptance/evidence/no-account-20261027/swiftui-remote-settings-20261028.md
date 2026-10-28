# 原生远程发布配置接线验证

## 范围与结果

Personal 隔离窗口测试通过。测试通过实际的 Swift `PublisherModel`、WebView 桥接和 Rust 桌面命令，使用合成内存密钥库验证：

- 分别保存预览与正式渠道配置，并读取完整字段。
- 拒绝不使用 HTTPS 的地址；重新读取后原配置仍然存在。
- 将预览渠道切回本地，再选回保留的远程目标。
- 新建 Swift 模型后重新读取两个渠道的配置。

测试使用临时 HOME、合成数据和内存密钥库。运行时沙箱禁止网络和真实密钥库访问。测试没有进行远程发布，也没有重启应用进程。

## 执行记录

```sh
python3 apps/desktop/swiftui/Tests/NativeWorkspaceFixture/run.py --edition personal --remote-settings --output /Users/gua/CodexWorkspaces/claude-switcher/output/no-account-20261027/native-remote-settings-20261028-r1
```

结果为 `PASS`，`remote_settings` 为 `true`。测试二进制 SHA-256 为 `45ef46c74378c8598937f39439e04059d5ff7d9fbb6baf2c7310bf24a0e6edef`。[结果与源码摘要](swiftui-remote-settings-20261028.json)随本记录保存。构建和运行日志保存在上述输出目录的 `build.log` 和 `runtime.log`。

本轮只增加测试接线。没有重建普通产品包，也没有操作真实 Keychain、真实 R2/CDN、实际安装或付款。真实窗口内手动操作和应用进程重启后的配置恢复仍需单独验收。
