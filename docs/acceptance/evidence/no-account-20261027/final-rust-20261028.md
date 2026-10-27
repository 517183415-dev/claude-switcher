# 2026-10-28：当前代码集中隔离验证

测试输入为提交 `6631b64b93bbf7649f450f1fce672f33e934edf6`；开始与结束时已跟踪文件均无未提交修改。测试按此前受控入口执行，使用独立 HOME、离线 Cargo 和 macOS Seatbelt。该策略允许本机回环，用于本地模拟服务；它禁止外网、真实资料目录、系统钥匙串服务及系统配置命令。策略文件 SHA256 为 `3e266f89c06e932f3d72cf2b6f74b9e6150accb4383d7ffa4947bc0b3fb362fa`。

1. 工作区：`cargo test --offline --locked --workspace --tests -- --skip unregistered_or_wrong_signer_service_never_authenticates --skip time::tests::real_macos_continuous_sample_is_available --test-threads=1`。顶层 962 项通过、0 项失败、10 项忽略、2 项跳过。3 项嵌套子进程结果不重复计入顶层。广告后台失败回归通过。[原始日志](final-rust-workspace-20261028.log)，SHA256 `7cace881307c4551754f77779c2b2228b3b6524fb864224958950256f4be5f74`。
2. 独立 Public：`cargo test --offline --locked -p switcher-desktop --no-default-features --features public -- --test-threads=1`。196 项通过、0 项失败、2 项忽略；合成签名内容接收、篡改拒绝与恢复回归通过。[原始日志](final-rust-public-20261028.log)，SHA256 `71eb1912d4f6e9ba8f3c19a1e3b20a5012212c7802fd526876ba8e67a07fb1c9`。该测试与工作区测试部分重叠，不相加。

忽略项包括子进程夹具、受控 APFS 测试、专门的跨进程入口和原生 Keychain 验收；两个跳过项依赖真实系统功能。忽略和跳过均不计入通过。此前有效的 Swift 测试、两版严格 Clippy 和候选包构建没有重复执行。代码图谱状态为 complete、306 文件、7559 节点、30182 条边、pending 0。

本次不证明真实 Claude 登录、默认 Keychain、实际安装、真实 R2/CDN、线上传播、macOS 全版本或 48 小时与 7 天长期运行。注册、部署和付款保持暂停。
