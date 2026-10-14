# Claude 切换器 (Claude Switcher)

一个 macOS 原生小工具：**启动前环境安检 + A/B 双账号一键切换 + 代理掉线自动熔断**。

专为需要严格网络环境一致性的 Claude 桌面版用户设计。任何一项安全检查不通过，Claude 就不会被启动。

![截图](docs/screenshot.png)

## 功能

- **11 项环境安全检查**（启动前强制执行）
  代理内核 / TUN 网卡 / 出口 IP 归属地 / IP 与上次一致性 / 时区 / 地区 Locale / IPv6 旁路 / DNS 隧道校验（Fake-IP）/ WebRTC·STUN 多节点泄漏探测 / 保镖守护进程 / 域名置顶规则
- **一键修复**：检查不过时自动拉起代理、恢复 TUN 开关、修正时区/IPv6/DNS、重装守护进程、重写分流规则
- **A/B 双账号切换**：完整快照隔离（独立 Device Fingerprint / OAuth Credential），切换即物理交换数据卷，一次登录长期免登
- **保镖（Kill-Switch）**：守护进程每 10 秒巡检，代理内核或 TUN 离线瞬间自动关闭 Claude，杜绝裸连重试
- **时区看门狗**：系统时区被改动时自动回滚并留痕
- **域名置顶路由**：Claude 全家族及生态域名（Turnstile / Datadog / Segment / Stripe / Intercom）置顶强制走代理，免疫第三方规则集误判

## 系统要求

- macOS（Apple Silicon / Intel）
- [Clash Verge](https://github.com/clash-verge-rev/clash-verge-rev)（TUN 模式可用的代理）
- Claude 桌面版（`/Applications/Claude.app`）
- python3（`xcode-select --install`）

## 安装

下载后双击 `installer/安装.command`，按提示操作（自动识别代理组名与时区，中途需要一次管理员密码）。

> 如提示"无法打开，因为来自身份不明的开发者"：右键 → 打开 → 打开。

## 使用

1. 双击桌面「Claude切换器」
2. 等待 11 项检查完成：全绿 → 选账号启动；有红项 → 点「一键修复」
3. 首次使用 B 账号：选择账号 B，在打开的 Claude 中登录一次即可，以后切换自动存档

## 项目结构（二改指南）

```
├── app/main.swift          # SwiftUI 界面源码(单文件,swiftc 直接编译)
├── core/
│   ├── claude-switch.sh    # 核心逻辑:安检/切换/修复(全部功能在此)
│   └── stun-probe.py       # STUN 泄漏探针(纯标准库手工实现协议)
├── daemons/                # launchd 守护进程模板(安装时物化路径)
├── installer/              # 双击安装/卸载程序
└── assets/                 # 图标源码(Swift 程序生成)与成品
```

**改界面**：编辑 `app/main.swift` 后重新编译：

```bash
swiftc -O -parse-as-library app/main.swift \
  -o "Claude切换器.app/Contents/MacOS/ClaudeSwitcher" \
  -framework SwiftUI -framework AppKit
```

**改检查项/加新检查**：编辑 `core/claude-switch.sh` 的 `checkdetail()`（UI 逐行渲染）与 `check()`（启动门禁）即可，键名与 UI 的 `keyMap` 对应。

## 安全说明

- 本工具只做本地环境管理，不修改、不注入 Claude 客户端，不与 Anthropic 服务器产生任何额外通信
- 所有网络探测仅发往 Cloudflare / STUN 公共服务器
- 账号存档（含登录态）保存在 `~/.claude-switcher/`，请勿手动删除

## License

MIT © Claude Switcher Contributors
