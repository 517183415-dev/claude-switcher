# R04 F04/F05 第一轮本地结果

状态：隔离验证及同一 ChatGPT 对话复核通过；R04 无剩余项。任务 `c2c_ad0b`，iteration 1。

## 改动与合同

F04 将 `repair_legacy_merge_at` 的返回值定义为“需要重载”。实时规则已通过时仍返回 false。配置或 Merge 文件已有规则、实时规则未通过时返回 true。两个分支不重写文件、不产生备份。缺失或待更新的 Merge 沿用既有备份、竞态检查和原子写。真实 `Native::merge` 调用 `reload_if_needed`，按 stop → start → verify 顺序执行，错误直接返回。

F05 拆出仅校验、检查运行及必要时打开应用的入口。它不占用 readiness 等待预算。完整 `execute_app_open_with` 仍保留原 40 次等待。真实旧版代理组合调用同一可注入实现，顺序为打开 → Verge TUN 修复 → 必要启动 → 单次 readiness 等待 → 出口验证。打开错误直接返回。事务前置和持久化后两次合法检查仍保留。现代 `regions.rs` 未修改。

## RED 与最终 GREEN

- F04 RED：2 方法均在重载行为断言失败，退出 101。源快照及哈希见 `f04-red-source.rs`、`f04-red-receipt.json`。
- F05 RED：9 方法，2 通过、7 行为失败。旧组合超时为 80 checks / 80 pauses；打开错误被吞掉。源快照及命令见 `f05-red-manifest.json`。
- 最终 F04 3 方法通过，实际 Merge 返回值接入重载 helper 的 4 个情形符合次数要求。
- 最终 F05 9 方法通过；超时 40 checks / 40 pauses，第 40 次就绪为 40 checks / 39 pauses；打开错误后 TUN、启动、等待、出口调用均为零。
- 三模块 34 + 6 + 15 = 55 个独立方法通过。上面的 F04/F05 已包含其中，不重复相加。离线 Clippy、三个文件的 rustfmt 和全差异检查通过。

`final_checks.py` 顺序执行八条最终命令。每条命令前后核对九个累计修改文件的 SHA256。R01–R03 六个已接受文件哈希保持一致。完整命令、退出码、非零测试数和哈希见 `verification.json`、`final-receipts.json`。

## 只读复核与边界

独立只读 reviewer 复核三个文件，未发现可操作阻塞。确认实际组合接线、完整等待入口、两次合法 gate，以及备份/竞态/原子写合同。

未安装产品或依赖。未访问真实账号。未操作真实应用、系统服务、系统设置或产品网络代理。实际安装、GUI、宿主运行验收均为 `NOT_RUN`，长期验收为 `WAIT`。未提交、推送或合并。
