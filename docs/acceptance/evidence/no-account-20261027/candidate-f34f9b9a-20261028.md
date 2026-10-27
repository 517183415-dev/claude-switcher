# 两版候选包：广告来源修复

Personal 和 Public 均从提交 `f34f9b9a` 的源码归档构建。两版包含广告来源状态和刷新提示修复。[构建记录](../../../../output/no-account-20261027/candidate-f34f9b9a.ws1GnR/build.log)显示两版完成；构建脚本对复制前后的应用包执行了 `codesign --verify --deep --strict`。

两版均为 arm64、版本 0.2.0，最低 macOS 版本为 13.0。签名为本地 ad-hoc 签名，不代表 Apple 公证。[校验凭据](../../../../output/no-account-20261027/candidate-f34f9b9a.ws1GnR/receipt.json)记录源码归档哈希、四个关键源码与提交的比对结果和各包可执行文件哈希。

构建使用已有依赖与离线 Cargo。为适配隔离环境，仅在归档副本的 `build.rs` 中给 `swiftc` 增加 `-disable-sandbox`；外层沙箱继续限制非本机网络和默认 securityd。该调整没有写入仓库源码。构建没有配置正式广告来源。

本次没有运行候选包的原生窗口，也没有安装候选包或验证默认 Keychain、真实 Claude、真实 R2/CDN 和公证。产物和日志保存在本机 output 目录，不包含在 Git 提交中。
