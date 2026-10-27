# 生产运营 IPC 的共享密钥边界与备份闭环

普通 Personal Builder 注册应用级 `Arc<ManagedVault>`，默认内含 `KeychainVault`。生产命令使用 Tauri State 注入；测试仅替换此边界为已有 MemoryVault，仍经 `ipc::handler`、实际参数解码、普通 ACL 和生产异步处理函数。Public 不注册运营密钥服务。DesktopState 保持独立，不增加依赖或前端 JSON 参数。

Publisher 生命周期锁先于 Vault 锁；完整恢复会话继续持有原有锁。Vault 锁中毒返回 SecretUnavailable，后续回调不得运行。合成构造器只编译于 test，无产品设置或 IPC 可切换到合成密钥库。

新增两项分发表业务测试和一项密钥边界测试：

1. 生成签名密钥、拒绝重复生成、导入并规范化图片、三槽六期、预览/正式发布、加密备份、暂停并发布较高版本、恢复旧备份、重建 App 状态、再次明确发布。独立校验 current 与 immutable 版本字节一致、验签、素材哈希和私有权限；恢复不改变公开目录，保留旧代次和旧密钥，重新发布超过恢复前正式最高序号。
2. 恢复准备/取消/重新准备/确认、保留代次列表及加密导出、同一目录拒绝重复导入；另一个隔离安装经生产便携备份恢复签名身份，再导入旧代次归档。独立比较旧 state 字节、当前 active/state、旧与新密钥；导入不发布，App 状态重建后保留列表一致。
3. 密钥边界 panic 后连续两次失败关闭，不运行后续业务。

`targeted` 首轮两项失败是测试预期错误：导入图片会去除元数据，保留代次归档不是完整恢复封装。`targeted-corrected` 第一项通过，第二项遇到正常的 archive_generation_exists 拒绝。修正测试使用另一个隔离安装后，`archive-corrected` 单项通过。以上失败不是产品 bug 的 RED；未为了通过而改动图片规范化或归档覆盖合同。失败日志保留。

阶段末 Personal 主程序 236 项、Public 主程序 35 项通过；两版四个集成目标各 140 次通过，同一专用 Seatbelt 用例各按原配置忽略一次，不计为通过。两版 all-target Clippy 和六个变更文件格式检查通过，保留三条既有 Personal 迁移风格警告及 ts-rs 提示。294 个源码/构建/测试输入在完整验证后无漂移。CodeGraph complete、pending 0：241 文件、6373 节点、26801 条边。

阶段末验证结果见 receipt.json。inputs.json 绑定 294 个生产、构建及测试输入（含新增测试文件）。最终检查后重新计算，源码不得漂移。命令、退出码、耗时与日志 SHA256 由 run.py 保存；Cargo offline/locked，使用既有 account-free.sb 和 ordinary-registry 合成 HOME/TMPDIR，真实用户资料与外网访问被阻止。

仅证明 MockRuntime、MemoryVault 与本机私有发布目录。App 状态重建不是操作系统进程重启。未证明真实 Keychain、WKWebView、真实 R2/CDN、其他设备传播、真实系统安装或长期稳定性；不把历史原生证据升级为当前候选。未执行注册、部署、付款或系统设置变更，未启动子代理。
