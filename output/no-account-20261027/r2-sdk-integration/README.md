# R2 工具包与发布流程的本地联通验证

日期：2026-10-27。测试使用独立 HOME、临时目录、合成密钥和本机模拟对象存储。Seatbelt 禁止外网、真实用户资料、默认 Keychain 和系统设置操作。它没有连接 Cloudflare。

`publish_configured_with` → 测试注入的 R2Store → AWS 官方 S3 工具包 → 本机模拟对象存储。模拟端记录公开读取和附带认证头的请求，并实现版本条件写入。配置、密钥读取、目标选择、发布锁、图片与版本上传、current 切换、公开回读均经过产品实现。模拟端只核对认证头格式与合成 access key，没有验证 AWS 请求签名；测试用 HTTP 回环地址替代实际 HTTPS/R2。

结果：

1. 两次预览发布成功，版本号递增。测试独立验证两个版本的 Ed25519 签名与图片内容；公开读取没有认证头。
2. current 的首次写入使用 `If-None-Match`，后续写入使用 `If-Match`。模拟另一写入改变 ETag 后，第三次发布返回冲突。原公开内容和两个成功历史记录保留。
3. 旧 R2 适配器定向集合 11 项通过；新增联通和延迟发送回归 2 项通过。[通过日志](integration-final.log)、[适配器日志](r2-tests.log)。
4. 一次复测暴露测试服务器的非阻塞读取问题：连接已建立但请求尚未到达时返回 `WouldBlock`，随后测试提前失败。[原始失败日志](targeted-test-final.log)。修复为阻塞读取，并加入延迟发送的回归测试。这个问题发生在测试夹具，不证明产品发生该问题。
5. Personal 测试目标 Clippy 通过，保留 `publisher/migration.rs` 三条既有风格警告和 ts-rs 提示。[日志](clippy.log)。五个改动 Rust 文件的格式及 Git 差异空白检查通过。完整套件没有重复运行。

[源码输入摘要](inputs.sha256)对应最终测试源码。[代码图谱状态](codegraph-status.json)为 complete、pending 0。

尚未验证真实 Cloudflare、TLS/CDN、防攻击规则、实际服务端请求签名校验、真实客户端自动传播或软件二进制更新。此项不证明 AD-03/AD-04 全部边界。注册、部署和付款保持暂停。
