# F01：账号存档覆盖风险的独立核实

日期：2026-10-25。交接编号：CS-GPTAUDIT-20261025-A1。

工作树：`/Users/gua/CodexHome/worktrees/36b7/claude-switcher`。
分支：`codex/gpt-audit-fixes`。
HEAD：`98c42fc6825ab90fe018032ae4243883e988d856`。

结论：采纳 F01，列为首项整改。置信度为高，仅限静态控制流。本轮没有运行 Rust 事务，没有构建、产品测试、安装或真实账号访问。没有观察到真实资料已经丢失。

## 直接证据

| 当前文件与行号 | 实际代码事实 |
|---|---|
| `crates/switcher-core/src/lifecycle.rs:126–145` | 切换先执行环境检查；通过后调用 select，然后再启动。前面的检查没有证明活动资料归属于 A。 |
| `crates/switcher-core/src/migration.rs:173–204` | store 在活动目录缺失时创建空目录，然后选择路径调用 select_legacy_checked。 |
| `crates/switcher-core/src/accounts.rs:379–402` | selected_account 在 current 与 active-state 都缺失时返回 A。 |
| `crates/switcher-core/src/accounts/legacy.rs:132–171` | first=true 不阻止 outgoing 复制。活动目录无条件复制为 outgoing。选择 A 时 in_place=true，不载入已存的 A。 |
| `crates/switcher-core/src/accounts/legacy.rs:184–206` | journal 将默认 A 写为 before.account，并记录已有 A 的目录身份。 |
| `crates/switcher-core/src/accounts/legacy.rs:274–316` | 已有 A 时，把 outgoing 与 A 存档交换，再把 previous_archive 交给 clean_stage。 |
| `crates/switcher-core/src/accounts/legacy.rs:321–344` | clean_stage 在身份符合时删除残留子目录。身份验证保护了删除对象，却没有证明用空资料替换 A 的业务依据。 |
| `core/claude-switch.sh:937–960` | 保留的旧实现仅在无 current 且活动目录非空时收编 A；否则无当前账号可归档，直接载入目标已有存档。 |

旧实现对照使用本提交保留的 shell 文件。上述行号不是报告中 original_HEAD 的历史行号。

## 静态状态推导

前提：没有 current、没有 active-state；活动目录为空或原本缺失；accounts/A 非空；没有其他恢复阻断；目录、锁和环境检查通过；复制与交换成功。

```text
没有账号归属记录
        |
        v
selected_account 默认返回 A
        |
        v
空活动目录 -> 空 outgoing
        |
        v
交换 outgoing 与 accounts/A
        |
        v
accounts/A 变空，旧 A 移入暂存
        |
        v
提交并清理暂存 -> 旧 A 目录被删除
```

选择 A：保留空活动目录，已有 A 不会被载入。
选择 B：载入 B 存档或创建空 B，但上述 A 归档覆盖仍发生。

这条推导使用当前函数的实际分支。它不是重新实现产品的模型测试，也不是 Rust 运行重现。环境预检失败会在选择前阻止该路径；不能把缺陷描述成每次开窗必然发生。

## 现有测试证明了什么

`accounts/legacy.rs:491–508` 的 Fixture 默认写入 `A-latest`。
`:517–548` 验证首次 A 和缺少同账号存档时保留活动目录身份。
`:553–592` 验证非空首次 A 在各提交边界中断后的恢复。
这些测试有保留价值，但上述两组测试不覆盖“空/缺活动目录＋已有 A＋两个状态文件均缺失”。本轮只审阅测试源码，未运行测试。

## 最小整改方向

1. 将未确认归属与显示默认 A 分开。默认显示不能授权覆盖已有存档。
2. 无归属且活动资料为空时，载入目标已有存档，或者进入空目标登录态；不生成覆盖 A 的 outgoing。
3. 无归属但活动资料非空时，保留旧版首次收编 A 的行为。明确 current 的既有切换语义另行锁定。
4. 保留锁、身份和内容校验、原位首次建档、旧 journal 解码与恢复。保持 accounts/A、accounts/B 存档结构。

预计业务文件范围：`accounts.rs`、`accounts/legacy.rs`、`migration.rs`，以及这些模块内的定向测试。具体 journal 表达在红灯用例明确后选择，不先重写整个账号模块。

## 下一轮验证

| 合成场景 | 必须得到的结果 |
|---|---|
| 空/缺活动目录 × 选择 A/B × 已有非空 A | 旧 A 内容始终可恢复；选 A 载入旧 A，选 B 不覆盖旧 A。 |
| 无状态＋非空活动资料＋首次 A/B | 保留旧版首次收编及选择后启动语义。 |
| 已明确 current、同账号、已有 v2 state | 不改变既有选择和兼容优先级。 |
| 新分支各持久边界中断后重复恢复 | 不删旧 A，不载入错误账号；旧 journal 继续可恢复。 |

运行状态：`NOT_RUN`。首次实际核实记录见 `F01-首次核实.json`。
