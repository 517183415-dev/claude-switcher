R08/F07：本地源码与隔离验证通过，等待 ChatGPT 复核。

仅改两个授权文件。未知/多客户端、0/N select组要求显式选择。完整 v2/validated legacy 配置零探测，冲突拒绝。选择进入既有 literals 与 digest；改变/缺少必要选择在锁、停止进程、写入前失败。普通 GUI 从已提交归档确认 owner/edition/generation，仅 install/replace/explicit finalize 在释放锁后请求一次。启动失败保留 durable committed；环境仍 NOT_RUN。

调用顺序：
```
plan → 选择与digest校验 → 加锁 → 组件提交 → 释放锁 → 请求普通GUI → 已有首次检查/R07修复入口
```

原始3行为RED与独立审查后3个注释方法RED均保留。最终18定向/178全量全部通过，定向包含在全量内，不能相加。git diff --check通过。30个保护函数AST与字节、其余14文件hash均未变化。真实GUI、产品安装/网络/账号/服务/系统操作未运行，长时验收WAIT。

执行边界异常：writer的一次全模块测试误设临时HOME，触发rustup syncing/downloading5components；exit1。该轮NOT_ACCEPTED。无法确认具体下载/安装完成量，临时HOME已清理、无Rust进程残留。原始日志、准确行号及hash在f07-boundary-violation.json。后续根会话固定已存在的1.96.0直接二进制与缓存，CARGO_NET_OFFLINE=true，禁止rustup/native/socket入口后重新完成最终验证。后续GREEN不抹去历史异常。

验收细节和实际命令见 verification.json、final-receipts.json、final-runtime.json、final-scenario-receipts.json。源差异按本轮baseline保留在f07-source-diff.patch。
