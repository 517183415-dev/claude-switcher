# Claude 切换器 · Linux 版 (Beta)

> ⚠️ **本版本为 Beta**。核心逻辑移植自 macOS 稳定版,但 Linux 发行版差异大,遇到问题请到 [Issues](https://github.com/517183415-dev/claude-switcher/issues) 反馈。

## 先说最重要的设计差异(为什么是这些取舍)

1. **Claude 官方没有 Linux 桌面版**——Linux 上的使用形态是「网页版(浏览器) + Claude Code CLI」。因此:
   - **账号切换**切换的是**浏览器登录态**(对 `~/.config/google-chrome` 或 `~/.config/chromium` 做快照交换),切换前会完全退出浏览器
   - **保镖**只终止 `claude` CLI 进程,**不会关你的浏览器**(网页版没有独立进程可归因,关浏览器体验太糟;网页端的保护由 TUN + 置顶规则兜底)
2. **没有图形界面**——Linux 桌面环境太多样,本版提供**终端交互菜单**(`claude-switch menu`)+ 完整命令行接口
3. **常驻服务用 systemd 用户单元**——免 root、登录自启、崩溃自动拉起

## 系统要求

> **注意**：macOS 版已支持多代理客户端（Clash Verge / Shadowrocket / sing-box / ClashX / Surge / Odcloud）与地区自适应（按出口国家自动锁定时区+地区格式，换节点后 `set-region` 一键重配）。Linux 版当前仍默认按美国出口设计，安装时会按 ipinfo 自动锁定时区；其他国家/地区的完整支持在路线图中。另外，自动更新（`update` 命令 + 12 小时自动检查）与 48 小时 IP 审查目前仅 macOS 版提供，后续版本同步。

- 任一现代发行版(Ubuntu/Debian/Fedora/Arch 等,systemd)
- [Clash Verge](https://github.com/clash-verge-rev/clash-verge-rev) Linux 版,TUN 模式可用
- python3、bash、curl
- 建议安装 `dnsutils`(Debian/Ubuntu)或 `bind-utils`(Fedora)以获得 dig;没有也能跑(自动回退 nslookup/getent)

## 安装

```bash
# 先开好代理(TUN + 美国节点),安装器会校验出口必须是美国
bash install.sh
```

- 安装器自动识别代理组名和时区(按出口 IP)
- 时区锁定和 IPv6 关闭需要一次 sudo 密码
- IPv6 关闭会持久化到 `/etc/sysctl.d/90-claude-switcher.conf`;恢复:删掉该文件后 `sudo sysctl -p`

## 使用

```bash
claude-switch menu          # 交互菜单(推荐)
claude-switch checkdetail   # 14 项逐项检查
claude-switch fix           # 一键修复
claude-switch switch A      # 切到账号A(切换浏览器登录态)
```

## 与 macOS/Windows 版的差异

| 项目 | Linux 版 |
|---|---|
| 账号切换对象 | 浏览器配置目录(Chrome/Chromium),不是独立 App |
| 保镖保护对象 | `claude` CLI 进程(不关浏览器) |
| 常驻方式 | systemd 用户服务(免 root) |
| 界面 | 终端菜单 + CLI |
| 时区/IPv6 | timedatectl / sysctl(需 sudo) |

## 卸载

```bash
systemctl --user disable --now claude-bodyguard.service
rm -f ~/.config/systemd/user/claude-bodyguard.service
rm -rf ~/.claude-switcher ~/.local/bin/claude-switch
sudo rm -f /etc/sysctl.d/90-claude-switcher.conf
```
