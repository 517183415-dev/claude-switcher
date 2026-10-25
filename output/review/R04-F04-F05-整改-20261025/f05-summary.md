# R04 / F05 切片证据

状态：实现已冻结；本片新增 9 项合成测试 GREEN。父会话负责最终完整回归、Clippy、格式检查与独立复核。

## 根因与最小修复

原 legacy Native::proxy 先调用包含 40 次就绪等待的 execute_app_open，再执行自己的 40 次等待；调用还丢弃 open 结果。已抽出验证时间和模式、检测运行状态并打开应用的 execute_app_open_only_with。legacy 实际生产组合使用该 helper 并传播错误；原 execute_app_open 仍调用完整等待包装，不变更 modern regions.rs 调用合同。

实际生产组合 proxy_with 的顺序为：

```text
validated open-only
    → Clash Verge TUN repair
    → optional start
    → one 40-check / 40-pause maximum readiness loop
    → country check
```

TUN flag 修复保留既有结果处理。execute_with 的前后两次高层 proxy gate 保留。Native::merge 接入 reload_if_needed(needs_reload, runtime)，其顺序仍为 stop → start → verify("")；F04 的 needs_reload 决策及集成测试由父会话负责。

## RED → GREEN

命令：`cargo test --offline --locked -p switcher-core --lib f05_ -- --nocapture`

| 阶段 | 结果 | 证据 |
|---|---|---|
| RED：仅抽 seam，保留两轮等待及丢弃结果 | 2 passed / 7 failed，exit 101 | f05-red.log、f05-red-manifest.json、f05-red-crates__switcher-core__src__*.rs |
| GREEN：open-only 与错误传播 | 9 passed / 0 failed，exit 0 | f05-green.log、f05-green-manifest.json |

| 合成条件 | RED | GREEN |
|---|---|---|
| 始终未就绪 | checks=80, pauses=80, country=0 | checks=40, pauses=40, country=0 |
| 第 40 次就绪 | checks=41, pauses=39, country=1 | checks=40, pauses=39, country=1 |
| open 失败 | 错误被吞，仍 tun/start/ready/country | 原错误立即返回；后续四类操作均为 0 |
| 真实隔离 TUN 文件需修复 | open→ready→tun→stop→start→ready→country | open→tun→stop→start→ready→country |
| open-only | 有 40 次检查及暂停 | checks=0, pauses=0 |

测试覆盖四个有应用名的模式、三个无需自动打开的模式、已运行状态、可选 start、四种无效时间条件、不支持自动打开的显式 plan、open 错误，以及真实 repair_legacy_tun_at 在临时目录的文件修改。计数和顺序来自同一个 Native::proxy 使用的组合函数，未以复制算法替代实际组合。

## 冻结源哈希

| 文件 | SHA-256 |
|---|---|
| crates/switcher-core/src/proxy_repair.rs | 67c9d0ce5f7515e64026d34601511008ef1c5c2eae2ed551db3fa26203e81fb3 |
| crates/switcher-core/src/regions/legacy.rs | 7ca6ecd950489f406ab21b143a07c27f1f2f7add37732e98a95e2db70b506d23 |
| crates/switcher-core/src/regions.rs（未改） | c07c470b805a7435171f7f06bc03745ec2349d1a7e1c34dbd72a0ed65162173f |

GREEN manifest 记录命令、退出码、前后相同的上述哈希。两份修改源码已经 rustfmt；本片 `git diff --check -- crates/switcher-core/src/proxy_repair.rs crates/switcher-core/src/regions/legacy.rs` 返回 0。

## 验证边界

只验证离线合成 Runtime/AppOpener 与临时 TUN 文件。没有打开、关闭或重启真实应用，没有真实网络、代理、系统设置或账号操作。检查次数与暂停次数证明有界算法，不证明真实网络墙钟耗时。现代 fullwait 既有回归、高层两次 gate 既有回归及统一 Clippy/格式检查由父会话最终验证，不能从本片 9 项结果直接声明这些检查通过。日志中的 ts-rs serde 属性解析 warning 原样保留。
