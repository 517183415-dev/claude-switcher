# 真实 WebView 运营成功路径：限定通过

本阶段在实际 WKWebView 内，经正式 `ipc::handler` 和原 capability 完成 17 次响应。它们不是 17 个独立测试。测试使用专用前端、合成 MemoryVault 和本机对象存储，不使用真实 Claude 账号或用户 Keychain。

1. **链路和结果**

`ipc-business --ordinary-publisher` 必须显式启用，并使用隔离合成目录。生产默认启动保持原行为，未注册测试命令或更改 ACL。窗口完成密钥创建/重复拒绝、规范化素材、三槽六期、预览和生产发布、加密备份、较高暂停版本、错误口令拒绝、恢复及显式续发。五份宿主检查点与独立 Node Ed25519 验证确认：签名有效，current 对应 immutable；素材摘要、尺寸及原始文件 0600 正确；错误口令不变；旧代次保留；恢复不自动发布；续发高于暂停序号。

[成功原生回执](r2/receipt.json)、[完整阶段摘要](summary.json)、[浏览器响应](r2/webview.json)。直接测试进程已 TERM 并 wait 回收；本地回执服务已关闭。系统托管 WebKit helper 的完整回收未由本工具证明。

2. **失败、修复和复用**

r1 release 构建成功，但工具没有预建 `tmp/business-root`，应用在窗口创建前以 101 退出。保留 [原始日志](r1/runtime.log) 和构建摘要，不计通过。`prepare_paths` 现先创建 0700 目录；回归确认路径、权限与拒绝复用非空目录。r2 复用 r1 的有效原生二进制，核对二进制摘要、产品输入及嵌入前端字节相同，没有再次编译。成功原生调用只执行一次。

3. **必要检查**

相关正式 IPC 集合 7 项通过；工具隔离 3 项和独立验收器 5 项通过。验收器拒绝替换响应、签名篡改、旧代次改变和恢复时公开指针改变。Personal `personal,ipc-business` Clippy、Public `public,ipc-business` 编译与四个 Rust 文件的 2024 格式检查通过。保留三个既有迁移风格警告和 ts-rs 提示。初次格式命令误选 2021，仅检查未写入，随后使用项目 2024 版本通过。[检查回执](checks/)、[工具执行记录](tool-tests.json)。

复用 c64b91b5 的 Personal 完整 236 项和 b2e6a78e 的 142 次原生 ACL 拒绝证据，不重复运行。r2 捕获的所有输入在阶段末无漂移；后加入的宿主验收器测试单独绑定于 [最终输入](final-inputs.json)。不把编译配置不同的旧完整结果称为本阶段测试特性完整套件。

4. **复现与证据保存**

从项目根目录运行：

```sh
python3 -m unittest discover -s apps/desktop/tests/native-publisher -p test_runner.py -v
python3 apps/desktop/tests/native-publisher/test_verifier.py --fixture output/no-account-20261027/native-publisher/r2 -v
# 仅需新一轮原生证据时运行，要求已解锁；使用全新输出目录：
python3 apps/desktop/tests/native-publisher/runner.py --output output/no-account-20261027/native-publisher/new-run
```

Git 不能保存 0600/0700。宿主测试仅在一次性复制目录中重建这些权限，再回放签名和状态检查。原生实时验证发生于原文件，没有改写被验证文件的权限。原始构建/运行日志、检查点、合成状态、加密合成备份、前端、配置和输入摘要随证据提交；不提交应用二进制、WebKit HOME 或编译缓存。受限环境禁止真实资料、Keychain、安全设置和外网，仅允许固定回环服务与 WebKit 必需解析器套接字。

5. **剩余范围**

默认 Keychain 权限、普通产品前端/启动、原生恢复两次确认与取消、真实 R2/CDN、完整 AD-10 和其他完整分支仍未通过。它们不会因本阶段升级为完成。下一步补原生两次确认和取消。UI 排最后，采用参考图玻璃质感，保持当前布局及非蓝紫色背景。注册、部署和付款暂停；本阶段没有启动子代理。
