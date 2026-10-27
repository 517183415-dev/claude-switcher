# 广告手动刷新桌面命令：隔离测试

## 测试范围

测试通过 Tauri MockRuntime 调用实际 `refresh_ad_content` 处理函数，并使用当前命令权限。临时工作区和合成密钥库独立于用户资料。测试时钟使用本机 UTC 对齐签发时间，并使用合成启动标识和连续时间；生产运行仍使用系统时钟。沙箱禁止外网，只允许回环连接，也禁止读取真实账号和默认 Keychain。

测试要求编译时的发行来源配置为空。它不会输出配置值；不满足该条件时直接失败。签名内容由测试中的 Personal 发布命令生成，并使用合成密钥。

## 结果

| 构建特性 | 结果 | 验证内容 |
| --- | --- | --- |
| Public | 1 项通过 | Personal 与 Public 状态下，关闭广告位和缺少来源时，命令返回对应提示。 |
| Personal | 2 项通过 | 同上；两版状态还验证本机签名发布、刷新、签名篡改拒绝和恢复后的再次刷新。 |
| 静态检查 | 通过 | Personal 测试目标 Clippy 使用 `-D warnings` 通过；变更的两个 Rust 文件通过 rustfmt；`git diff --check` 无错误。 |

定向命令分别使用 `cargo test -p switcher-desktop --no-default-features --features public --bin switcher-desktop ad_refresh_dispatch -- --nocapture` 和 `--features personal`。Cargo 使用离线模式运行。

首次 Personal 测试错误地期望显示两个活动；实际只有一个活动处于当前时段，另一个尚未开始。修正测试预期后重跑通过。这是测试预期错误，不是发现的产品缺陷。

[Public 日志](../../../../output/no-account-20261027/ad-ipc-20261028.hzZtUb/public-final.log)、[Personal 日志](../../../../output/no-account-20261027/ad-ipc-20261028.hzZtUb/personal-final.log)、[首次失败日志](../../../../output/no-account-20261027/ad-ipc-20261028.hzZtUb/personal.log)、[Clippy 日志](../../../../output/no-account-20261027/ad-ipc-20261028.hzZtUb/clippy.log)。Clippy 日志含现有的 ts-rs 宏解析提示。

## 未验证

签名成功的 Public 状态运行在 Personal 特性测试程序中，不代表独立 Public 特性构建的签名成功路径已通过。测试没有使用发行来源配置、原生窗口、默认 Keychain、真实 Claude 或真实 R2/CDN。它也不证明客户安装和线上传播。既有核心及远程测试本轮没有重复运行。
