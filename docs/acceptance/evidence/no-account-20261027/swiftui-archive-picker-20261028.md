# SwiftUI 归档选择回归

## 范围

导入旧归档与恢复加密备份使用同一文件选择和口令交接逻辑。取消、读取失败或空文件不会提交操作，并保留当前输入。成功读取非空文件后清空输入，再交给原有模型操作。界面消失时的清空行为未改。文件读取仍拒绝符号链接、非普通文件和超限文件。

## 测试

- 先将原来的口令清空顺序提取成共用函数，不改变其行为。四项新测试中，取消、读取失败和空文件三项失败；成功交接一项通过。[RED 日志](../../../../output/no-account-20261027/archive-picker-20261028.BOgR4z/red-r2.log)。
- 修改顺序并拒绝空文件后，四项新测试和五项相关既有测试共 9 项通过。[GREEN 日志](../../../../output/no-account-20261027/archive-picker-20261028.BOgR4z/green.log)。
- 首次测试尝试因 SwiftPM 在外层隔离内再次启用沙箱而报 `sandbox_apply: Operation not permitted`，测试未启动。[记录](../../../../output/no-account-20261027/archive-picker-20261028.BOgR4z/red.log)。重试仅关闭 SwiftPM 的内层沙箱；外层继续拒绝网络和 `com.apple.securityd`。

测试使用合成选择结果和合成口令。它不操作真实文件选择面板，不证明实际 App 窗口交互或默认 Keychain。没有连接真实 Claude、R2 或 CDN，也没有重建或安装产品包。注册、部署和付款保持暂停。
