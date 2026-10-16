# Claude 切换器 · Windows 版 (Beta)

> ⚠️ **本版本为 Beta,未经大规模真机测试**。核心逻辑移植自 macOS 稳定版,但 Windows 环境差异大,遇到问题请到 [Issues](https://github.com/YOURSJEYO/claude-switcher/issues) 反馈。

## 系统要求

- Windows 10 / 11 (64 位)
- PowerShell 5.1+（Windows 自带,无需安装）
- [Clash Verge](https://github.com/clash-verge-rev/clash-verge-rev) Windows 版,且 **TUN 模式可用**
- Claude 桌面版 Windows
- 安装时需要**管理员权限**一次（注册守护任务 / 锁定时区 / 关闭 IPv6）

## 安装

1. 先把 Clash Verge 开好（TUN 模式 + 美国节点）——安装器会校验出口必须是美国
2. 右键 `install.ps1` →「使用 PowerShell 运行」,或在管理员 PowerShell 中执行
3. 如果提示禁止运行脚本,先执行一次:
   ```powershell
   Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
   ```
4. 按提示走完,桌面会出现「Claude切换器」快捷方式

## 与 macOS 版的差异（注意事项）

| 项目 | 说明 |
|---|---|
| 时区名称 | Windows 用 `Eastern Standard Time` 这类 ID,安装器会自动把 IP 城市映射过去 |
| 保镖频率 | 计划任务登录启动,隐藏窗口常驻,每 10 秒巡检(与 Mac 一致) |
| IPv6 | 关闭的是所有网卡的 ms_tcpip6 绑定;要恢复: `Enable-NetAdapterBinding -ComponentID ms_tcpip6` |
| SmartScreen | 首次运行可能弹"Windows 已保护你的电脑" → 点「更多信息」→「仍要运行」 |
| 账号数据目录 | Windows 版在 `%APPDATA%\Claude`(与 macOS 路径不同,逻辑相同) |
| 卸载 | 计划任务里删掉 ClaudeBodyguard,删除 `%USERPROFILE%\.claude-switcher` 和桌面快捷方式即可 |

## 已知限制

- 界面是 WPF 重写的第一版,样式以 Mac 版为准对齐,细节可能有出入
- Clash Verge 的 TUN 开关在 Windows 上如果被更新重置,目前需要手动在设置里重新打开(一键修复会提示)
- 账号快照切换前请确保 Claude 已完全退出(托盘图标也要退出)
