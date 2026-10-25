# R06 P2 非 UTF-8 参数最小修复

只修改 `crates/switcher-core/src/bin/claude-switcher.rs`。repair_cli.rs 保持原冻结 hash。此前 root unified 与原 GREEN 证据均未覆盖。

## 根因与变更

main 原先在读取 `--legacy-cli` 之前转换全部 OsString。任意参数转换失败时，固定调用 Modern 的 finish_result，因此 legacy 参数错误落到 stdout JSON。现在 main 调用可注入 backend 的 run_raw_cli；raw 参数阶段按已有“存在精确 --legacy-cli”规则保存错误输出模式。UTF-8 转换成功时仍调用原 run_cli；Invocation 参数位置、重复 flag 与业务参数校验保持原样。

## RED / GREEN

- `cli-nonutf8-red-source.rs` 保留旧转换失败 Modern 行为，并包含新测试。
- `cli-nonutf8-red.log`：编译通过；1 方法失败；cargo exit 101。5 个实际 receipts 全部零 backend 调用；3 个 legacy case 都观测到错误的 stdout JSON。
- `cli-nonutf8-green.log`：CLI f06_ 8/8；40 个实际 receipts。
- `repair-nonutf8-green.log`：repair f06_ 5/5；10 个实际 receipts。
- `cli-nonutf8-freeze.json`：rustfmt check / git diff check 均 exit 0，并含源码 hash、精确命令与行号。

新测试通过 Unix OsStringExt::from_vec([0xff]) 直接驱动生产 run_raw_cli。覆盖 modern、modern --json、固定 legacy、错位 legacy、重复 legacy。收集全部用例后断言精确 stdout/stderr/code；所有用例断言零 backend。legacy stdout 为空、stderr 为 invalid_command 换行、code 2；modern 保持原 JSON stdout、stderr 空、code 2。

## 边界

没有启动真实 CLI 子进程、真实 checks、应用、服务、网络或账号操作。测试验证真实 main 所调用入口的内存 sink，不声称真实安装 shim 的非 UTF-8 进程端到端验证。原真实账户 helper 隔离临时 fixture 测试继续通过。旧 ts-rs deny_unknown_fields warning 仍存在；编译与过滤测试通过。
