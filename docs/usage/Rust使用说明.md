# Rust 使用说明

## 1. 当前交付

本项目提供 GitHub 源码和本地构建／安装脚本。无需购买 Apple 开发者会员或商业证书。

用户于 2026-10-23 要求恢复旧版核心行为。2026-10-27 用户选择现行 PRD 的检查与启动证据规则，覆盖旧恢复计划中的对应决定。SourceLocal 不自动更新。保留四个 Rust 包、Personal／Public 和暖白墨绿界面。恢复历史见[执行计划](../plans/旧版核心逻辑恢复执行计划-2026-10-23.md)。

以下按现行合同说明使用流程，不代表全部源码路径或本机安装已验证。悬空current拒绝及CLI普通fix复检已有定向隔离回归，CNB的适用Rust整组复验已通过；分层状态见[当前进度](../handoff/无需账号功能进度-2026-10-27.md)。历史CNA/v4结果单独保留。历史ipc-smoke仅覆盖三个命令。CNC新增独立ipc-limits测试入口，两版preflight与Personal图片/备份六个载荷边界限定通过；图片实际导入，备份仅验证closure入口计数，未解密或访问Vault。整项A01及普通业务IPC仍未验。历史记录中的 r17 与本轮候选不同，本轮未读取当前安装状态。只有实际安装验证完成后，才能把源码结果归为本机安装结果。真实登录与 A→B→A 单独验收。状态见[实施状态](../handoff/Rust-R4-实施状态.md)。

当前产品CNC：`23c9964403e904ab44c2cc334d40992454193221`，tree：`920c47bf92fc69ce8916266f61327b3766c8fada`。普通CNC两版App包已独立构建并完成静态验证。每版为22条检查及2条复制/签名变更；Public构建后两版各12条目重验PASS。准确路径与哈希见[当前CNC候选绑定](../handoff/无需账号功能进度-2026-10-27.md#0-当前cnc阶段)。新A01标记均无；旧E01字符串保留REVIEW_MARKERS_PRESENT，静态解释不证明普通App动态IPC。该测试入口不供日常使用，普通App未启动或安装；历史候选包保留。

## 2. 日常使用

1. 打开应用。程序自动检查 14 项环境条件。
2. 查看检查原因。处理失败项后重新检查。
3. 点击账号 A 或 B。程序预检、退出 Claude、切换资料并启动 Claude。
4. 启动成功后，切换器约 2.2 秒退出。
5. 操作失败时，查看提示和活动记录。账号事务中断时使用恢复入口。

```text
打开应用 → 自动检查 → 选择 A/B → 退出 Claude → 切换资料 → 启动
                        ↓                       ↓
                   首次非空资料收编 A       首次 B 使用空资料
```

首次活动资料非空时，可按首次收编规则保存为 A。归属未知且活动目录为空或缺失时，不得覆盖已有 A。没有 B 存档时，程序创建空资料，供用户首次登录。账号判定优先使用安全有效的 current A/B 标记；标记缺失或内容不识别时兼容 v2 状态回退。不安全文件（含悬空 current 符号链接）拒绝；只有目录项真正缺失才按缺失规则回退。需要回退时，损坏的 v2 记录拒绝。选择当前账号仍需检查并满足启动门禁。

普通切换不要求额外安装健康收据、迁移备份证明、固定 Claude 版本或 Keychain 调查收据。程序仍先退出 Claude，避免在资料写入时交换账号。既有存档与中断现场保留。

一次环境检查的总预算为30秒。STUN全部无响应时，结果为Unknown，并拦截启动。IP历史提示和ASN提示不单独阻止启动。启动证据必须不超过30秒，且网络代次一致；超过30秒或换网后重新检查。程序在启动前再次复核，证据不满足条件时不能启动。

## 3. 修复、代理和守护

“环境检查”可以显示修复计划。“设置 → 目标地区”可以修改锁定国家、时区和地区格式。CLI 的 fix 和 set-region 默认直接执行；--preview 仅预览。

CLI普通fix成功执行后，完整复检14项，且只复检一次。复检阻断时返回非0退出码，不输出OK。现代CLI保留预览JSON，最终JSON包含report；旧兼容输出增加FIX:check及完整报告，只有通过才输出FIX:complete和OK。预览或准备/执行失败不触发该复检。此次修复未改变set-region与GUI语义；GUI原有成功后复检继续保留。

代理未运行时，程序按应用名称尝试打开。Shadowrocket、ClashX、Surge、Clash Verge、sing-box 和 0dcloud 保留旧兼容路径。打开 Shadowrocket 不保证 VPN 已连接。Clash 退出等待约 3 秒，启动后最多约 40 秒等待代理与 TUN。

普通用户偏好和代理配置由用户进程处理。时区、IPv6、系统服务和 DNS 缓存操作按需申请管理员授权。授权取消后，程序报告错误，并继续可完成的用户项。set-region 先保存地区目标和用户 locale，再申请系统授权，沿用旧版顺序。

root 保镖沿用固定旧模板，约每 10 秒检查代理和 TUN。失败时按 Claude 路径匹配并发送 TERM，保留其他登录用户的旧停止范围。Rust 用户 guardian 同时保留。root 时区看门狗启动即检查，之后每小时纠正目标时区。

## 4. 构建、安装和更新

在项目根目录执行：

```bash
bash scripts/build-rust.sh personal
# 或构建 Public
bash scripts/build-rust.sh public
```

脚本构建本机架构的 App、Rust CLI 和后台组件。安装步骤见[本地安装说明](../../packaging/macos/README.md)。安装器备份已有组件和旧命令配置，支持覆盖替换。检查、启动、切换和用户级修复不要求每次重新登记本机代码指纹。

本机安装保留受保护的系统组件和 root 服务。更换这些组件时，macOS 需要管理员授权。日常普通功能无需因此重复安装。按用户要求，本轮集中完成构建和无需密码的验证，最后统一安装。

SourceLocal 不自动更新，也不接受远程二进制替换。升级时拉取所需源码，再构建和安装。Personal 手动更新、Public 自动更新的可信发布源分支仍保留；它们不构成本轮源码交付的材料门槛。

广告保持已有三个广告位和本地运营实现。本轮不扩展远程托管或投放。详情见[广告运营说明](广告运营说明.md)。

## 5. 日常 CLI

安装器将 ~/.local/bin/claude-switch.sh 交接为 Rust CLI 入口。入口保留旧配置字面量，并调用已安装应用中的 Rust 可执行文件。它不执行旧 Bash 核心。未安装时，可直接使用本机构建的 Rust CLI。

| 命令 | 行为 |
| --- | --- |
| claude-switch.sh check | 输出 OK 或 FAIL，退出码表示预检结果 |
| claude-switch.sh checkdetail | 输出 14 项逐行结果，保存 last-check.txt；--json 获取 JSON |
| claude-switch.sh status | 只读查看当前账号和存档状态 |
| claude-switch.sh switch A 或 B | 预检、退出 Claude、切换并启动 |
| claude-switch.sh fix | 成功修复后完整复检14项一次；阻断时非0且无OK；--preview仅预览 |
| claude-switch.sh set-region | 按出口重新锁定地区；可指定 us、jp、tw、hk、sg、gb、de、fr、kr、au、ca |
| claude-switch.sh recover | 恢复中断的账号事务 |
| claude-switch.sh diagnose-proxy --config 文件 | 独立代理诊断；传输失败返回退出码 1 |

Rust CLI 的 --edition personal 或 --edition public 位于命令前；安装入口自动选择已安装发行。start 重新检查并启动当前账号。diagnose-proxy 不创建账号，不修改系统，也不授予启动许可。

旧事务尚未完成时保留现场。使用对应恢复入口后重新检查。真实恢复、真实登录和连续运行均需单独记录结果，隔离测试不能代替这些验证。
