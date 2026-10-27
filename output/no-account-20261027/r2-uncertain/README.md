# R2 发布响应丢失的本机恢复验证

日期：2026-10-27。测试使用独立 HOME、临时目录、合成密钥和本机模拟对象存储。Seatbelt 禁止外网、真实用户资料、默认 Keychain 和系统设置操作；没有连接 Cloudflare。

预览配置走 `publish_configured_with → Publisher → R2Store → AWS S3 SDK → 本机模拟存储`。模拟端在成功保存 `current.json` 后关闭连接，不返回响应。SDK 测试配置将请求次数限定为一次。发布器返回结果未知，保留待处理记录和发布锁，没有记入成功历史。测试在同一进程中销毁并重新打开 Publisher；再次执行发布后，它认出相同的已签名版本，匿名回读 current、不可变版本与图片，保留一个版本和一条历史，并释放发布锁。测试独立验证 Ed25519 签名。

三项相关测试通过：[测试日志](tests-final.log)。Personal 测试目标的 Clippy 通过，保留 `publisher/migration.rs` 三条既有风格警告与 ts-rs 解析提示：[日志](clippy.log)。仅修改测试夹具和测试，没有修改产品运行逻辑。该测试没有重启操作系统或应用进程，也没有验证写入前断连、真实服务端请求签名、真实 TLS/CDN、Cloudflare CAS 和线上传播。AD-04 整体验收仍待验证。

[源码输入摘要](inputs.sha256)对应最终测试源码；[代码图谱状态](graph-status.json)为 complete、pending 0。注册、部署和付款继续暂停。
