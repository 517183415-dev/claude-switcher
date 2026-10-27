# R2 地址校验与发布说明回归

本阶段只使用合成数据、现有离线工具链与回环 HTTP。沿用 `../storage-directory-race/account-free.sb`，拒绝外网、真实账号目录、凭据和系统修改工具。没有注册、部署或付款，没有启动子代理。

## 缺陷与修复

1. `R2Config::validate` 用字符串解析 IP，遗漏带方括号的 IPv6。`red-r2.log` 中 `https://[::1]` 被接受，回归断言失败。修复使用 URL 解析器的 `domain()` 类型判定，在配置、身份计算和 Store 打开共用入口拒绝所有 IP 字面量。三个 IPv6 输入覆盖回环、映射 IPv4 和普通 IPv6；原 IPv4、合法域名、提供商域名及尾点测试保留。`r2-green.log` 的 11 项通过。
2. 运营页发布卡片仍声称“仅本机发布”，与已经接通的按渠道 R2 发布矛盾。`ui-red.log` 保留从生产 Svelte 片段实际渲染出的错误说明。修复发布说明和历史渠道标签。新增回归编译并渲染生产发布及历史片段，覆盖两个渠道、已验证和待确认记录。页面布局、导航和按钮位置未改。

## 复现与证据

- `red.log` / `red.json`：首轮 Cargo 在运行测试前被隔离环境的 Git 排除配置读取阻断。不能作为缺陷复现。
- `red-r2.log` / `red-r2.json`：切换合成 HOME 和独立 Git 配置后的有效 RED。
- `r2-green.log` / `r2-green.json`：修复后整个 R2 模块定向验证。
- `ui-red.log`：原生产发布卡片的渲染回归失败，4 通过、1 失败。之后扩充同一用例的历史标签断言。
- `verify.py`：阶段末一次检查入口。运行 `python3 output/no-account-20261027/r2-origin-ip-literals/verify.py`。使用固定 Rust 1.96.0、Cargo offline/locked、合成 HOME 和现有沙箱。
- `verification.json`：命令、逐项退出码、耗时、输入前后 SHA256。各检查原始日志同名保存。此前定向测试包含在完整套件内，不能相加为独立用例。

这是本地校验和受控回环传输证据。它不证明真实 R2、IAM、TLS/CDN/WAF、Keychain、普通 App 完整动态 ACL 或客户端实际传播。普通 App 的既有证据仍绑定 `9d6a3efcfc6e907ae7797633a843720533a94b34`，本阶段未重建普通双包。

## 阶段末结果

阶段末检查：桌面后端 224 项、前端 64 项及两组 VM 回归通过；Svelte 检查 0 错误/0 警告，两版前端构建、Public 编译和格式检查通过。Clippy exit 0，保留 3 条既有迁移风格警告。R2 定向 11 项已包含在完整套件内，不累加。

本轮 276 项跟踪输入前后 SHA256 一致。结果详见 `receipt.json` 和 `verification.json`。

原始 `.log` 按仓库忽略规则保留在本地。Git 保存回执、输入绑定、日志 SHA256 和复现脚本。
