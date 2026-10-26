# Claude 切换器当前代码图谱

更新日期：2026-10-27，Asia/Shanghai。项目根目录：`/Users/gua/CodexWorkspaces/claude-switcher`。

## 1. 基线与索引范围

当前开发分支为 `codex/rust-rewrite`。整合基线C49为 `49c2615beac0e1f7ac7352977d3212229fe9b012`，已包含 `codex/gpt-audit-fixes` 的全部代码。整合交付记录为 `84d7bc1b0b0d80630745272361b6aaa41c9259fa`。历史产品修复CNA为 `0297efe2da236427b9a95a584aba53c4f30ce634`。当前产品CNB为 `7851ce10bd00a040bf8c209fd6e763baf63546ff`，tree为 `27d4826b204d079f3a28c48a7f2089d31fe1992e`。CNB补足CLI普通Fix复检和current悬空链接拒绝，保留既有检查、门禁与测试时钟成果。

CodeGraph 1.6.0 已完成增量同步。当前索引有 **159 个文件、4,530 个节点、17,725 条关系**。状态为 `complete`，待同步文件与待解析引用均为零。统计表示自动索引范围，不表示产品测试覆盖率。

根目录 [codegraph.json](../../codegraph.json) 排除 `output/`、`tmp/`、验收证据与审核副本。历史源码和证据原件均保留。`experiments/` 保留索引，但降低查询优先级。这样可避免冻结源码参与当前调用链分析。

自动索引覆盖 Rust、Svelte、TypeScript、JavaScript、Python、Swift 和 Objective-C。Shell、PowerShell、配置和文档仍须按入口单独核对。索引数据库位于本机 `.codegraph/`，不纳入 Git。旧同步记录保留；当前验证见[CNB后续摘要](../acceptance/evidence/no-account-20261027/branch-followup.json)，[CNA/v4摘要](../acceptance/evidence/no-account-20261027/summary.json)保留历史。

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

`main.rs` 用编译特性固定 Personal/Public，禁止同时启用或同时缺失。Public 不注册 Personal 的运营写入命令。`ipc-smoke`使用独立编译特性，只注册runtime_info、forbidden_probe、finish_smoke三个命令。A01真实IPC仍NOT_RUN；静态包检查不证明原生IPC已运行。

桌面 `DesktopState` 汇集快照、检查会话、账号动作、修复计划、地区预览和广告刷新。CLI与guardian复用核心业务模块。账号切换通过 `lifecycle::switch_checked`，当前账号启动通过 `lifecycle::launch_checked`。`checks::run_checks` 将前后网络采样和14项检查纳入同一30秒预算。`checks::evaluate_launch` 统一校验报告完整性、有效期和网络代次。`lifecycle` 独立读取当前代次，在账号复核后再次调用统一门禁，再提交系统启动。

账号current读取使用symlink_metadata，悬空符号链接拒绝，不再回退。CLI普通Fix成功apply后执行完整14项检查恰好一次；blocked/error非0且无OK。现代最终JSON带report；legacy带FIX:check和完整report。set-region及GUI语义不变。

广告和更新运行层通过函数指针接收已有 `ClockSample`。生产默认使用 `time::sample`；合成测试不再间接读取真实sysctl。没有新增IPC、环境变量或远程时钟覆盖入口。guardian描述符验证复用原权限谓词，特殊权限位拒绝规则不变。

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

C49历史整合回归保留125个方法、2组VM和44项包静态检查。历史CNA/v4执行Rust671次、567个不同名称，完整保留。当前CNB的v5本轮新执行679次、575个不同名称、0失败，新增8个方法；共享模块重复编入不能计为独立方法。Python200方法、前端60测试及2组VM和类型检查结果复用，未新跑；7个Python输入哈希未变。当前fmt与Clippy通过。原生时钟和已安装XPC连接两项Rust测试单列NOT_RUN；Python三项排除原因见本轮记录。

当前候选原生GUI/IPC仍为 `NOT_RUN`。实际安装、真实账号、实际外发、平台与长期运行按各自验收记录执行。完整隔离条件未满足时，不启动普通候选试探。后续有限范围审查仅有限推进，未证明35项完整分支覆盖，不报告未经计算的覆盖率。详见[本轮进度](../handoff/无需账号功能进度-2026-10-27.md)和[37项验收](../acceptance/当前候选验收-2026-10-27.md)。

## 5. 历史实现与产品依据

`app/main.swift` 是旧版 SwiftUI 入口，通过已安装的 `claude-switch.sh` 调用检查、切换与修复。`core/`、`daemons/`、`installer/` 保留旧版实现。`windows/`、`linux/` 保留各自平台实现，不能据此宣称当前 Rust 候选已通过跨平台验收。

`experiments/p0/` 是早期验证。审核包、冻结源码和验收证据用于历史复现，不能作为当前产品调用入口。

暖白、墨绿和横向工作台的视觉方向保留。页面、发行差异和交互以[产品 PRD v1.2](../plans/PRD-Rust双版本产品-v1.2.md)为准。[概念图](ui-concept-v1.png)仅提供视觉依据，不证明运行状态。
