# Claude 切换器当前代码图谱

更新日期：2026-10-27，Asia/Shanghai。项目根目录：`/Users/gua/CodexWorkspaces/claude-switcher`。

## 1. 基线与索引范围

当前开发分支为 `codex/rust-rewrite`。产品代码基线为 `49c2615beac0e1f7ac7352977d3212229fe9b012`。该提交已包含 `codex/gpt-audit-fixes` 的全部代码。整合交付记录提交为 `84d7bc1b0b0d80630745272361b6aaa41c9259fa`。本轮只更新索引配置与文档。

CodeGraph 1.6.0 已完成增量同步。当前索引有 **159 个文件、4,481 个节点、16,974 条关系**。状态为 `complete`，待同步文件与待解析引用均为零。统计表示自动索引范围，不表示产品测试覆盖率。

根目录 [codegraph.json](../../codegraph.json) 排除 `output/`、`tmp/`、验收证据与审核副本。历史源码和证据原件均保留。`experiments/` 保留索引，但降低查询优先级。这样可避免冻结源码参与当前调用链分析。

自动索引覆盖 Rust、Svelte、TypeScript、JavaScript、Python、Swift 和 Objective-C。Shell、PowerShell、配置和文档仍须按入口单独核对。索引数据库位于本机 `.codegraph/`，不纳入 Git。同步验证见[图谱更新记录](../acceptance/evidence/integration-20261027/codegraph-update.json)。

## 2. 当前入口与调用链

```text
桌面：src/main.ts → App.svelte／RegionSettings.svelte
                             │ Personal：Operations.svelte
                             ↓ Tauri IPC
       src-tauri/main.rs → commands.rs → bridge.rs／DesktopState
                             │ Personal：publisher_commands.rs → publisher/
                             ↓
CLI：claude-switcher.rs ─→ switcher-core ←─ switcher-guardian.rs
                             │
             ┌───────────────┼────────────────┐
             ↓               ↓                ↓
      检查与账号生命周期   修复与地区设置     广告与更新规则
             │               │                │
             ↓               ↓ 授权 XPC       ↓
      账号档案与事务日志   switcher-helper    本地状态／受信内容
                             ↓ 身份与策略校验
                         固定系统操作

switcher-domain：共享类型与协议 → Rust 模块／生成的 TypeScript 类型
packaging/macos：候选工件 → 安装计划 → 管理员安装／恢复事务
```

`main.rs` 用编译特性固定 Personal/Public，禁止同时启用或同时缺失。Public 不注册 Personal 的运营写入命令。`ipc-smoke` 使用独立编译特性；静态包检查不证明原生 IPC 已运行。

桌面 `DesktopState` 汇集快照、检查会话、账号动作、修复计划、地区预览和广告刷新。CLI 与 guardian 复用核心业务模块。账号切换通过 `lifecycle::switch_checked`，当前账号启动通过 `lifecycle::launch_checked`。检查、退出、交换、复检和启动的规则由核心实现负责。

helper 根据安装记录校验调用身份，再按固定策略执行系统操作。SourceLocal 与 DeveloperId 的更新许可由安装身份和可信发布配置共同决定。Public 的发行标识不能单独证明远端更新可用。

## 3. 模块与职责

| 路径 | 当前职责 |
| --- | --- |
| `apps/desktop/src/` | Svelte 页面、状态呈现、用户动作和生成的类型引用 |
| `apps/desktop/src-tauri/src/main.rs`、`commands.rs`、`bridge.rs` | 应用入口、IPC 注册、桌面状态与核心调用 |
| `apps/desktop/src-tauri/src/publisher/`、`publisher_commands.rs` | Personal 草稿、签名发布、备份、历史整理与素材回收 |
| `apps/desktop/src-tauri/src/update_runtime.rs` | 桌面更新状态、来源约束与更新动作协调 |
| `crates/switcher-domain/src/` | 检查、账号、发行、广告、系统操作等共享类型；导出 TypeScript 类型 |
| `crates/switcher-core/src/checks.rs`、`network.rs`、`diagnostics.rs` | 14 项检查、网络证据和独立代理诊断 |
| `crates/switcher-core/src/accounts.rs`、`migration.rs`、`lifecycle.rs` | A/B 存档、兼容状态、事务恢复、切换与启动门禁 |
| `crates/switcher-core/src/user_repairs.rs`、`regions.rs`、`guardian.rs` | 修复计划、地区变更和用户级守护；复用各自内部辅助模块 |
| `crates/switcher-core/src/ads.rs`、`ad_runtime/`、`updates.rs`、`signed.rs`、`time.rs` | 内容校验、广告运行、软件更新协议、签名和时间连续性；`ad_runtime.rs` 为运行入口 |
| `crates/switcher-core/src/bin/` | CLI、guardian 和 Keychain 元数据探针入口 |
| `crates/switcher-helper/src/` | 授权、XPC、调用策略、系统操作、安装健康与恢复 |
| `packaging/macos/` | 候选包、本地安装、取消、卸载和事务恢复工具 |

依赖方向保持“界面与入口 → 核心 → 共享协议”。特权动作经过 helper 的身份与策略边界。维护时优先复用现有规则，不在界面复制账号、检查或更新决策。

## 4. 测试入口与当前证据

| 入口 | 验证对象 |
| --- | --- |
| `crates/switcher-core/src/` 内测试、`crates/switcher-core/tests/` | 业务规则、事务、检查、诊断与隐私回归 |
| `crates/switcher-helper/src/` 内测试 | 授权策略、系统动作校验与恢复规则 |
| `apps/desktop/src-tauri/src/publisher/*tests.rs`、其他 Rust 内测试 | 发布、备份、整理、回收及运行状态 |
| `apps/desktop/src-tauri/tests/` | 桌面接口相关的核心准备、修复与导出回归 |
| `apps/desktop/tests/`、`apps/desktop/scripts/` | 前端行为、时间转换和既有隔离回归 |
| `packaging/macos/test_*.py` | 假安装、取消、卸载和恢复；使用隔离文件与替身命令 |
| `tests/` | 旧版 Shell 的切换安全与独立代理诊断回归 |

整合回归已完成 125 个测试方法及 2 组 Node VM 检查。类型检查为零错误、零警告。两版候选包共通过 44 项静态检查。这些结果来自本地整合记录，本轮文档更新未重跑产品测试。

当前候选原生 GUI/IPC 仍为 `NOT_RUN`。实际安装、真实账号、实际外发、平台与长期运行按各自验收记录执行。完整隔离条件未满足时，不启动候选来替代隔离验收。详见[本地整合交付](../handoff/本地整合交付-2026-10-27.md)和[37 项当前候选验收](../acceptance/当前候选验收-2026-10-27.md)。

## 5. 历史实现与产品依据

`app/main.swift` 是旧版 SwiftUI 入口，通过已安装的 `claude-switch.sh` 调用检查、切换与修复。`core/`、`daemons/`、`installer/` 保留旧版实现。`windows/`、`linux/` 保留各自平台实现，不能据此宣称当前 Rust 候选已通过跨平台验收。

`experiments/p0/` 是早期验证。审核包、冻结源码和验收证据用于历史复现，不能作为当前产品调用入口。

暖白、墨绿和横向工作台的视觉方向保留。页面、发行差异和交互以[产品 PRD v1.2](../plans/PRD-Rust双版本产品-v1.2.md)为准。[概念图](ui-concept-v1.png)仅提供视觉依据，不证明运行状态。
