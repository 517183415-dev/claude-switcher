# 两版候选包：2026-10-28

Personal 和 Public 均从提交 `e12ffd432282834ee33e525ba81b2fc6cff0d556` 的源码归档构建，包含归档选择口令保护和活动记录多行脱敏修复。[构建记录](../../../../output/no-account-20261027/candidate-e12ffd43.Bwatky/build.log)显示两版均完成；构建脚本对复制前后的应用包执行了 `codesign --verify --deep --strict`。

两版均为 arm64、版本 0.2.0，最低 macOS 版本为 13.0。签名为本地 ad-hoc 签名，不代表 Apple 公证。源码归档哈希、关键源码和 `Cargo.lock` 一致性、各包可执行文件哈希记录在[校验凭据](../../../../output/no-account-20261027/candidate-e12ffd43.Bwatky/receipt.json)。

构建使用现有依赖与离线 Cargo。为适配隔离环境，只在归档副本的 `build.rs` 中给 `swiftc` 增加了 `-disable-sandbox`；外层沙箱仍禁止非本机网络及默认 securityd。该调整没有写入仓库源码。

没有安装或启动这两版候选包，也没有验证窗口、真实 Claude 账号、默认 Keychain、真实 Cloudflare/CDN 或公证。构建产物和日志保存在本机 output 目录，不包含在 Git 提交中。
