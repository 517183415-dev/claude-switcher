# R06/F06 installer 切片完成记录

结果：本切片 GREEN。只修改 `packaging/macos/local_install.py` 与 `packaging/macos/test_local_install.py`。源码已冻结。未改其他代理的文件；未安装依赖或产品，未访问真实账号/代理/网络，未操作宿主应用、服务和系统设置。所有安装/恢复调用使用原有临时目录、元数据和进程假件。真实子进程仅隔离 bash/fake executable 与既有 F03 Rust test bridge。

## 修复与证据

- `local_install.py:119` plan 为新 install/replace 明确记录 `cli_contract=2`。archive/result 自然持久化。手工历史 fixture 缺字段继续是 v1，没有无意升级。
- `:970` cli_contract 仅接受整数 1/2，字段缺失才选择 1。bool、float、string、null、999、list、dict 一律 ValueError。
- `:978` cli_shim 的 contract 是必填参数。v1 保留历史完整字节；v2 只在固定 --edition 后增加 --legacy-cli。原始参数仍通过 `"$@"` 透传。
- `:268` plan_uninstall 从已通过原 generation/record/archive proof 的 archived plan 选择唯一 renderer。完整 bytes/owner/mode/group 校验不变，未加入 v1 OR v2 宽松分支。
- `:1059` cli_install、`:1074` cli_verify、`:1087` cli_restore 均使用当前 transaction plan。恢复继续允许原有 None/核验过的原备份/本事务唯一生成模板；没有扩大为任意另一版本。新 v2 的旧备份本身可以是 v1，这属于恢复原字节，不等于忽略生成版本。

## 两条核验调用链

```text
verify(owner)
  ├─ pending health → verify_components(j) → cli_verify(j.plan.contract)
  └─ committed → verify_components(archived) → cli_verify(archived.plan.contract)

verify_public(owner, edition)
  → 原组件/守护/进程核验
  → installed_cli_plan(installed record)
       → source_local generation / bounded stage / root metadata
       → archived record equality / stage / committed / owner / edition / cli_changed
  → cli_verify(archive plan 的唯一 contract 与原 literals)

plan_uninstall → 原代际 proof → archived plan contract → 唯一完整 shim bytes
```

RED 新发现：旧 verify(owner) 的 committed/pending 分支完全不检查 CLI；旧 verify_public 从 live shim 的 literals 重建期望值，既不读代际合同，也不拒绝 archive proof 漂移。父会话已确认这两处可纳入本文件边界。现在 verify_public 使用受保护的已安装 generation archive；不会依据 live bytes 猜版本。原 R02 proof 条款与七个受保护函数没有改动。

## RED / GREEN

| 运行 | 方法数 | 结果 | 时间 | 证据前缀 |
|---|---:|---|---:|---|
| baseline 生产 + 新行为测试 | 6 | exit 1；54 assertion failures、4 实际 ValueError | 1.136 s | installer-red |
| 首轮定向 | 6 | exit 0 | 1.402 s | installer-targeted-green |
| 首轮完整模块 | 159 | exit 0 | 43.448 s | installer-full-green |
| 补充 pending journal 矩阵后定向 | 7 | exit 0 | 0.971 s | installer-final-targeted |
| 最终完整模块 | 160 | exit 0 | 39.394 s | installer-final-full |

RED 的 4 个 errors 都来自生产拒绝合法 v2 shim 的 ValueError，不是编译、导入或 fixture 错误。RED 生产 hash 为 baseline b8f6dfc5f164346ae27158258579845ea86aa7e9dc55a126a858b6ead2618e4c。每次运行均保存独立 stdout/stderr、原始 command、exit、时间与前后源码 hash；RED 两份源码和最终两份源码均已留存。pending 新增用例为补充 GREEN 覆盖，不冒称属于原 6 方法 RED。

矩阵覆盖：3 种有效历史/显式版本 × 2 种 live bytes × 4 路径 = 24 个验证格；8 种无效合同 × 4 路径 = 32 格；pending journal 22 格；restore 22 格；archive stage/pending/owner/edition/record proof 漂移 10 个拒绝检查。四条路径是 cli_verify、verify_public、verify(owner archived) 和 uninstall preview。所有拒绝核验对 fixture 字节/metadata identity 的只读检查通过。

真实 bash fake 回归包含固定 --edition public --legacy-cli、check --json、空参数、空格、单双引号、字面量 `$()`、分号和中文。fake app 路径带空格/中文/单引号/$。子进程 stdout 132 bytes、stderr 17 bytes，exit 7 完整保留；原始结果保存为 `shim-fake.stdout`、`shim-fake.stderr`、`shim-fake.json`。没有执行传入字符串。

## 冻结与限制

`installer-source-freeze.json` 保存七个 R02 protected AST 的 expected/actual，7/7 完全相同。`installer-function-delta.json` 列出此次相对 R06 baseline 的函数变化。`installer-production.diff` 仅含本轮生产差异。R03 TARGET-only 解析/所有权函数、R02 holding/prepared/recovery/inode 逻辑未更改。

最终 SHA256：
- local_install.py: `fbe4c2ecbc37d2775bb2fd5c1626d20f3c4e97cebc910bf1d189bd345114ba3e`
- test_local_install.py: `f7bf3b561d2b3ceb3c2882f0eb52c5e0a6975f5db5282a91d31dae668bb9a3f4`

真实产品安装/卸载/系统恢复：NOT_RUN。v2 shim 依赖另一切片提供 --legacy-cli 解析；此处 fake executable 证明参数与流透传，不单独证明真实 Rust CLI 输出合同。缺失或损坏 generation archive 会使 public verify 拒绝，而不是退回从现场字节推断版本。整体 R06 的独立复核由父会话完成。
