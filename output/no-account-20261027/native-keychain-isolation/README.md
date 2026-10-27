# 独立 Keychain 原生恢复验证

范围：真实 Security.framework、正式 KeychainVault 类和 Publisher，明确指定本次创建的两个私有 Keychain。只有测试构造器可以选择私有库；正式 ManagedVault 默认选择、服务名和默认库读写 SDK 路径保留。提取共用编码函数，删除创建与更新的序列化重复。

已通过一次显式原生测试，包含：两个私有库的同名条目隔离；重复创建拒绝且旧内容保留；两个真实线程竞争同一 SecItemAdd、恰好一次成功且回读赢家；无效密钥/超限凭据拒绝且旧条目保留；加密备份、错误口令拒绝、成功恢复、旧代次与旧密钥保留、新代次回读及同进程重新打开。恢复不自动发布。一个测试名包含这些断言，不将断言数量计作独立测试数。

条目逐项清理并回查缺失，两个私有库删除；无遗留 Keychain 文件，条目清单为空。受限进程前后默认库及搜索列表的系统返回值与对象快照一致。只读偏好查询不枚举密码项；未调用默认库或搜索列表设置 API。Keychain 授权弹窗保持禁用。

前阶段默认库首次创建失败的证据完整保留在 `../native-keychain/`。本阶段先运行 Swift 小探针，确认显式私有库可创建、读写和删除，且前后偏好快照一致，再改正式 Vault。Apple 来源与下载哈希见 research-sources.json；完整研究输入保留本机。第一个编译失败为 Rust 2024 的 unsafe extern 声明，修正后构建通过，原始结果保留。

构建、普通回归和 Clippy 使用原 account-free 策略。显式原生执行只解除 securityd mach-lookup 阻断；真实资料、外网和系统操作的禁令保留。HOME/TMPDIR 位于本阶段目录，运行环境为白名单。

`run.py build ...` 构建并记录测试二进制；`run.py native` 使用全新 synthetic-state 私有目录运行唯一原生用例。不要覆盖已有 synthetic-state 或旧证据。private-probe.swift 是独立前置探针源码。receipt.json 记录最终检查结果，evidence-manifest.json 绑定提交的证据内容；二进制、Cargo 全量消息、合成状态和完整研究源码不提交。

此次没有证明用户默认 Keychain 的实际访问权限、系统进程重启、成功 WebView 业务、真实 R2/CDN 或客户端传播。原生库内并发/恢复通过不升级 AD-10 整项。没有使用真实 Claude 账号。注册、部署、付款暂停；SwiftUI 重写仍最后执行，保留当前布局和非蓝紫色配色。

集中验证：Personal 主程序 236 通过、原生默认忽略 1；Personal 全目标 Clippy 通过，保留三条既有迁移风格警告；Public 全目标编译通过；两文件 rustfmt 通过。未重复未受影响的前端或 Public 测试。显式启用保护复用的前缀比较见 reused-guard-evidence.json。
