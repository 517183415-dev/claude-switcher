# AGENTS.md —— Agent 交接提示词

> 这份文件写给 AI Agent。用户把本项目交给你时，请按本文档执行安装与配置。
> 用户也可以直接把本文件内容粘贴给任意本地模型（Kimi Code / Cursor / Claude Code 等）作为指令。

---

## 你的任务

帮助用户在 macOS 上安装并配置「Claude 切换器」：一个带环境安检的 Claude 账号启动/切换工具。目标状态是：**用户双击桌面图标，11 项安全检查全部通过，可以正常启动 Claude**。

## 开始之前：必须先确认的前置条件

逐项检查，缺哪项就先补哪项：

1. **macOS**（Apple Silicon 或 Intel 均可）
2. **Clash Verge** 已安装且代理可用（配置目录应存在于 `~/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/`）
3. **Claude 桌面版** 在 `/Applications/Claude.app`
4. **python3** 可用（`python3 --version` 能输出版本号；没有则引导用户运行 `xcode-select --install`）
5. **当前出口 IP 必须已经是美国**——先跑 `curl -s https://www.cloudflare.com/cdn-cgi/trace | grep loc=`，返回 `loc=US` 才能继续。不是美国就停下，让用户先把代理调好（安装程序会按出口 IP 锁定时区，裸连时装机会把时区锁错）

## 安装步骤

首选引导用户运行安装器（双击 `installer/安装.command`，或终端执行它）。如果安装器环境受限，按下面的手动等效步骤执行：

1. 把 `core/claude-switch.sh` 复制为 `~/.local/bin/claude-switch.sh`（可执行），把 `core/stun-probe.py` 放到 `~/.claude-switcher/`
2. 从 Clash 配置自动识别代理组名：在 `clash-verge.yaml` 的 `proxy-groups:` 下找第一个 `type: select` 的组名，写进脚本顶部的 `PROXY_GROUP="..."`
3. 按出口 IP 城市确定锁定时区（`curl -s ipinfo.io/json` 的 timezone 字段），写进脚本顶部 `TZ_TARGET="..."`
4. 参照 `daemons/` 目录的模板，生成保镖与时区看门狗脚本（把 `__HOME__` / `__TZ_TARGET__` 替换为实际值），并安装对应的 launchd 服务到 `/Library/LaunchDaemons/`（需要 sudo）
5. 编译界面 App（需要 Xcode 命令行工具）：
   ```bash
   swiftc -O -parse-as-library app/main.swift -o Claude切换器.app/Contents/MacOS/ClaudeSwitcher -framework SwiftUI -framework AppKit
   ```
   图标已在 `assets/` 里，打进 `Contents/Resources/AppIcon.icns` 并在 `Info.plist` 声明 `CFBundleIconFile`
6. 把 `installer/` 里的置顶规则写入 Clash Verge 的「全局扩展配置」(Merge.yaml)，重启 Clash Verge 使生效
7. 时区与 IPv6：`systemsetup -settimezone <TZ_TARGET>`、`networksetup -setv6off Wi-Fi`（需管理员）

## 验收标准（全部满足才算装完）

- 终端执行 `~/.local/bin/claude-switch.sh checkdetail` 输出 11 行且全部 `=ok`
- 双击桌面 App，界面全绿，能正常启动 Claude
- `launchctl print system/local.claude-bodyguard` 与 `system/local.tz-watchdog` 均能查到（服务已注册）
- 切换一次账号 B 再切回 A，A 的登录态完好

## 排障手册（以下全是实战中踩过的坑，按症状直接查）

| 症状 | 根因 | 处理 |
|---|---|---|
| TUN 网卡消失、出口变回国内 | **Clash Verge 更新/异常重启会把 `enable_tun_mode` 重置为 false**（生产环境已发生 4 次） | 退出 App → 改 `verge.yaml` 为 true → 重启 App。切换器的一键修复已内置此逻辑 |
| 时区总是自己变回去 | macOS 自动时区或未知程序改动（生产环境已抓 8 次） | 看门狗会自动回滚；查 `~/Library/Logs/tz-watchdog.log` 留痕 |
| 检查结果显示 0/11、数据为空 | 中文日志被 `cut -c` 按字节截断产生非法 UTF-8，Swift 严格解码整体失败 | 脚本必须 `export LC_ALL=en_US.UTF-8`；界面端用容错解码 `String(decoding:as:)` |
| 修改 App 后界面没变化 | 旧进程没死：App 关窗不退 + `pkill` 匹配中文路径会因 Unicode NFD 编码失配而静默失败 | 用 `pkill -x ClaudeSwitcher`（按进程名）；App 必须实现 `applicationShouldTerminateAfterLastWindowClosed` |
| headless Chrome 做 WebRTC 检测无结果 | headless 模式 ICE 子系统不产出候选 | 不要走浏览器路线，用本项目的纯 Python STUN 探针 |
| 代理节点报错但流量没泄漏 | 节点被重置时隧道内请求失败≠泄漏；Fail-Closed 下流量只断不绕 | 换线路即可 |
| 切换账号后对方看到"新设备" | 正常：快照隔离意味着每个账号有独立设备指纹，这正是设计目标 | 无需处理 |

## 安全红线（Agent 必须遵守）

1. **绝不在出口非美国时启动 Claude**——任何时候先验 `loc=US`
2. 切换账号前必须先完全退出 Claude 进程，再交换数据目录
3. 不要在日志、文档、Issue 里暴露用户的住宅/代理 IP 与任何凭据
4. 不要改动 `~/.claude-switcher/accounts/` 下的存档结构

## 文件索引

- 核心逻辑：`core/claude-switch.sh`（安检/切换/修复全在这一个文件）
- 泄漏探针：`core/stun-probe.py`
- 界面：`app/main.swift`（单文件 SwiftUI）
- 守护进程模板：`daemons/`
- 安装器：`installer/`
