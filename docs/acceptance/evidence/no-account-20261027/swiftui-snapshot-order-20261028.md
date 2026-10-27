# SwiftUI 快照乱序与广告设置

源码提交：`d7c1d4ad1d5f3dc6ebaf9271d895e94b3f4436d8`。测试使用合成传输，不访问真实账号或远程服务。

1. 两项回归在修复前失败：旧快照覆盖新快照；关闭广告后，旧快照重新显示广告和缓存图片。既有检查报告乱序测试同时通过。
2. 修复后，Swift 测试共 33 项通过。广告设置等通过 `beginAction` 启动的操作会作废此前未完成的快照；并发快照只允许最新请求更新界面。测试证明的是模型状态，不是实际窗口操作。
3. Personal 和 Public 的 arm64 产品包均已重新构建。复制后的包通过本地 ad-hoc 签名检查，最低系统版本均为 macOS 13.0；没有公证。新包没有重新运行窗口探针。

首次重建在外层沙箱内失败：Swift 编译器试图再次启动沙箱，系统返回 `sandbox_apply: Operation not permitted`。重试保留外层沙箱，并临时为 Swift 编译器加入 `-disable-sandbox`。构建结束后，构建文件已恢复并与提交逐字节一致。构建使用本地生成文件和依赖缓存，不是从 Git 归档构建。

[修复前日志](../../../../output/no-account-20261027/swiftui-snapshot-race-20261028/red.log)、[修复后日志](../../../../output/no-account-20261027/swiftui-snapshot-race-20261028/green.log)、[首次构建失败日志](../../../../output/no-account-20261027/swiftui-snapshot-race-20261028/package-personal.log)、[重建与哈希回执](../../../../output/no-account-20261027/swiftui-snapshot-race-20261028-retry/receipt.json)。

真实账号、默认 Keychain、实际安装、真实 R2/CDN 与线上传播仍未验证。注册、部署和付款继续暂停。以上结果不代表 35 项标准全部通过。
