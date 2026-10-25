# R06 / F06 本地整改记录

当前结果：本地隔离检查和 ChatGPT 第二轮复审通过。独立代码复核无剩余 blocker/P1/P2。接受结论见 `chatgpt-review-iteration2.txt`。

任务 `c2c_87c3`。iteration 1 只做 PLAN 预检，没有源码修改。iteration 2 执行修订后的五文件计划。基线 HEAD 为 `98c42fc6825ab90fe018032ae4243883e988d856`。累计 tracked diff 为 13 个路径，没有提交、推送或合并。

## 最小修改

| 文件 | 修改 |
|---|---|
| crates/switcher-core/src/accounts.rs | 仅公开已有只读 helper，并增加三行文档。函数体及剩余文件字节不变。 |
| crates/switcher-core/src/bin/claude-switcher.rs | 固定位置 legacy 模式；真实 recorded ownership；输出和退出码；可注入 handler 与双输出 sink。 |
| crates/switcher-core/src/repair_cli.rs | 同一个 coordinator 的输出模式与真实阶段 sink。保留现代 run 默认。 |
| packaging/macos/local_install.py | 新计划明确 v2；旧代际缺字段仅 v1；核验与恢复使用对应计划唯一模板。 |
| packaging/macos/test_local_install.py | 合成代际矩阵、pending/restore、真实 bash 到假程序的参数与流透传。 |

```text
legacy shim → 固定模式解析 → 现有业务处理 → stdout / stderr / exit
                     ↑
              原始参数先保留错误模式
```

## 输出合同

| 调用 | 输出 | 退出码 |
|---|---|---:|
| legacy status | current=A/B/none；hasA/hasB 来自独立 saved flags | 0 |
| legacy check 成功 | OK:unavailable；可附已有 warning reason | 0 |
| legacy check 阻断 | FAIL 加已有 launch reasons | 1 |
| legacy switch | 完成才输出 OK；业务失败输出 FAIL | 0 / 1 |
| legacy fix | 真实 FIX 阶段；执行完成才输出 complete/OK；失败无 OK | 0 / 1 |

Legacy 参数错误使用 stderr、code 2。Modern 保留现有协议。显式 check/checkdetail --json 优先。裸 binary check/checkdetail 保留原合同。set-region 保留现代协议。

CheckReport 没有结构化原始 IP。本轮使用明确 `unavailable` 占位。没有解析 reason 猜 IP，也没有新增网络查询或 DTO 字段。

独立复核发现非 UTF-8 参数转换丢失 legacy 模式。新增一项真实行为 RED 后，仅把模式保存放到转换之前。五个 raw OsString 场景全部零 backend 调用。该 P2 已关闭。

## 证据

RED：CLI 7 方法中 6 项失败；repair 5 方法中 4 项失败；installer 6 方法出现 54 个断言失败及 4 个合法 v2 被旧 renderer 拒绝的错误。非 UTF-8 补充 RED 编译成功，1 项失败。所有 RED 保留命令、退出码、原始日志和当时源码。编译失败没有被用作行为 RED。

最终 14 条命令全部 exit 0。去重后 255 个方法：CLI 8、repair 模块 9、installer 模块 160、accounts 59、migration 12、lifecycle 5、两个只读 CLI 集成测试。repair 定向 5、installer 定向 7 和非 UTF-8 定向 1 已包含在完整组中，不重复计数。Clippy all-targets、rustfmt、diff check 通过。

40 条 CLI receipts 捕获生产 handler 的真实内存 stdout/stderr/code。10 条 repair receipts 捕获生产 wrapper 的 stdout sink、Result 和调用次数。repair 库没有 stderr 接口；其中 modern Err→code 2 是主程序合同映射。该边界没有被冒称为运行真实主进程。

所有最终命令前后累计 13 个源码哈希一致。此前 11 个 accepted 路径中，8 个完整哈希不变。accounts 仅 visibility/doc 变化。installer 两文件按本轮合同更新。R02 七个关键函数 AST 不变。R03 TARGET-only 比较逻辑不变。

`final-receipts.json` 是最终检查索引。`verification.json` 记录计数、哈希和未运行项目。`output-matrix.json` 保存紧凑输出矩阵；较长值保留 SHA256，原始完整值在日志中。旧统一检查封存于 `pre-p2-unified/`，不作为修复 P2 后的最终结果。

## 验证边界

真实安装、卸载、账号、网络、DNS、代理、宿主应用、服务和系统修改均 NOT_RUN。真实安装 shim 到 Rust 的完整端到端链路 NOT_RUN；两端分别以真实 bash/假程序和 Rust 合成 backend 验证。48 小时/7 天运行验收 WAIT。没有安装依赖，没有访问真实账户资料。
