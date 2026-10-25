# R06 / F06 CLI 与 repair 切片

状态：两个源码文件已冻结。过滤行为测试 GREEN；父会话继续统一 Clippy、累计回归和独立复核。

## 改动

- `crates/switcher-core/src/bin/claude-switcher.rs`：固定全局位置解析 `--edition <edition> --legacy-cli`；status/check/switch/fix 使用兼容输出，合法 `check --json` / `checkdetail --json` 优先；直接 binary 的裸 check/checkdetail 和现代 JSON 行为保留。
- `crates/switcher-core/src/repair_cli.rs`：保留 public run 的 modern 默认。新增模式与 Write sink，NativeRepair 仍调用真实 coordinator 的 preview / execute；fake backend 使用同一个生产 wrapper。

没有修改 lifecycle、checks、regions、domain DTO、依赖、安装逻辑或账号 helper 函数体。accounts 可见性由父会话独立负责。

## 输出合同

| 场景 | stdout | stderr | code |
|---|---|---|---:|
| legacy status 未记录归属 | current=none hasA=0 hasB=0 | 空 | 0 |
| legacy status 已记录 A/B | current=A/B；hasA/hasB 来自真实状态 | 空 | 0 |
| legacy check 放行 | OK:unavailable；有 warning 时追加竖线与 reason | 空 | 0 |
| legacy check 阻断 | FAIL:launch reasons | 空 | 1 |
| legacy switch 完成 | OK | 空 | 0 |
| legacy switch gate/selection/launch 失败 | FAIL:已有 outcome.message | 空 | 1 |
| legacy 非法参数 / 错位重复 flag | 空 | 固定参数错误 | 2 |
| modern 异常 | 现有 error JSON | 空 | 2 |

`OK:unavailable` 是明确降级合同。CheckReport 没有结构化 IP。代码只显示 warning reason，不提取 IP，不增加网络/DNS请求。测试特意将保留地址 198.18.0.1 放在 warning 中，证明地址仍属于完整显示文本，不成为 OK 后的地址字段。

repair legacy 顺序：

```text
FIX:prepare → 真实 preview
                 ├─失败→FAIL / 1
                 └─成功→FIX:preview
                           ├─preview-only→0（无 apply / complete / OK）
                           └─FIX:apply→真实 execute
                                         ├─失败→FAIL / 1
                                         └─成功→FIX:complete → OK / 0
```

set-region 在两种入口下都保持现代 JSON。对失败状态只格式化已有结果，不伪造更细进度。

## RED 与最终 GREEN

```sh
cargo test --offline --locked -p switcher-core --bin claude-switcher f06_ -- --nocapture --test-threads=1
cargo test --offline --locked -p switcher-core --lib repair_cli::tests::f06_ -- --nocapture --test-threads=1
```

| 切片 | 行为 RED | 最终 GREEN | receipts |
|---|---|---|---:|
| CLI | 1 passed / 6 failed，exit 101 | 7 passed / 0 failed，exit 0 | 35 |
| repair | 1 passed / 4 failed，exit 101 | 5 passed / 0 failed，exit 0 | 10 |

RED 是保留旧输出行为、仅抽入 backend/sink 后的真实失败。没有把编译错误当行为 RED。保存文件为 `cli-red-source.rs`、`repair-red-source.rs`、对应 red.log / red-manifest.json / red-receipts.json。

最终冻结复测使用 `cli-frozen-green.*` 和 `repair-frozen-green.*`，早前 GREEN 保留为历史。各 JSON receipts 来自测试 stdout 中的实际内存 sink 输出，含 stdout/stderr/code/calls。repair 的 preview_calls / execute_calls 来自 backend 的 prepare/apply 计数；每个 apply 分支各一次，preview-only 和 preview 失败不调用 execute。

repair library wrapper 不接受 stderr sink，也不写 stderr；其 receipt 的 stderr 为空是该 wrapper 的结构性边界。modern Err 保留为 Result::Err，并按 main 原合同记录 code=2；该 library receipt 不冒充已运行 main 的最终 error JSON。binary wrapper 的 stdout/stderr/code 均用 Vec 捕获，modern runtime error JSON/code2 已单独验证。

## 账号只读证明

binary 测试直接调用真正公开的 `AccountStore::inspect_with_recorded_account`。输入全部位于自建临时目录：empty、已有 root+active 但无记录、recorded A、recorded B。前两者 display current=A，但 recorded=None，legacy 输出 current=none。A/B 读取真实合成 current marker。

每次检查后，root/active 存在性、current marker 内容、active sentinel 内容保持不变；未创建 v2/accounts。测试后只删除自建临时目录。未改 HOME、未读真实账号、未运行真实检查。

## 冻结

| 文件 | SHA-256 |
|---|---|
| crates/switcher-core/src/bin/claude-switcher.rs | 8a2c8243050d9eb3b244f123cffeda9a337d179ac38f111df8d1c7146178705f |
| crates/switcher-core/src/repair_cli.rs | 959f34138a5bef7bfbf196ae341bb66d27727f3110bfe7549703ccb971ed97e1 |

`cli-rustfmt-receipt.json`：rustfmt 与 rustfmt --check 均 exit 0，前后哈希相同。此后两条过滤测试再次 GREEN。两文件 git diff --check 返回 0。精确函数位置记录在 cli-source-anchors.json。

## 边界与剩余风险

本片仅验证合成 backend 和临时只读账户元数据。没有运行整个 checks_cli、实际 HOME、真实 checks、网络/DNS/loopback、应用、服务、系统设置、安装或依赖命令。Native backend 的实际 macOS 行为不在本轮证据内。未知归属仍为 None；没有把显示默认 A 变成所有权事实。通用 YAML、真实用户配置等均不属于此切片。

统一 Clippy、已接受功能回归和独立复核由父会话执行，本片没有提前宣称通过。新增公开接口仅传模式和输出 sink；私有 backend seam 不构成一般 command 框架。
