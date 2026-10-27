# 密钥恢复跨进程验收

状态：PASS_NATIVE_PRIVATE_KEYCHAIN_WRITER_EXIT_THEN_NEW_READER。

协调进程创建一个全新私有 Keychain。写入进程 55307 调用正式 Publisher / KeychainVault，完成加密备份与恢复，退出码 0；协调进程确认退出并回收后，才启动读进程 55325。读进程重新打开库与 Publisher，核对恢复代次、recovery_required、旧密钥和恢复后的密钥、旧代次状态摘要、未自动发布，退出码 0。

一个验收测试通过，内部两次 worker 执行不重复累计为独立功能数。新增协调器和 worker 默认忽略 2 项；缺少显式启用参数时，在创建根目录前拒绝。条目清单为空，私有库已删除，受限进程前后默认库和搜索列表快照一致。

首次编译出现依赖引用、SDK 参数类型和可变借用错误，修正为复用既有摘要函数及正确 SDK 签名，没有新增依赖。第一次原生执行因系统创建的临时 Keychain 文件权限宽于断言而失败；修正为仅收紧本次私有文件到 0600。保留原严格权限断言作为回归保护；RED 的 native-failed.log 与清理结果、GREEN 的 native.log 均保留。失败轮私有库也已删除。编译原始 JSON 日志留本机，提交精简错误摘要及带原日志摘要的回执。

定向编译、Personal bin/tests Clippy、两文件 rustfmt 均通过。Clippy 保留三条既有迁移风格警告。产品源码和依赖不变，沿用 c64b91b5 的 Personal 236 项完整测试及 Public 编译证据，不重复运行前端或原生 ACL 矩阵。verify.py 只核对保存的日志、文件摘要、退出码、清理结果与源码差异；不会重跑测试或访问 Keychain。

范围：直接 Publisher 与显式私有 Keychain；不证明用户默认库权限、成功 WebView 业务、整机重启、实源 R2/CDN 或完整 AD-10。UI 仍最后执行，保留当前布局和非蓝紫色背景；注册、部署、付款暂停。

```text
协调器 → 私有库 → 写入进程 → 退出 / wait 回收
                → 新读进程 → 回读并验证 → 退出 / wait 回收
       → 精确清理 → 默认库及搜索列表快照核对
```

入口：apps/desktop/src-tauri/src/publisher/native_keychain_process_tests.rs。
复核：python3 output/no-account-20261027/native-keychain-process/verify.py。
回执：[receipt.json](receipt.json)。实际退出证据：[writer](synthetic-state/writer-exit.json)、[reader](synthetic-state/reader-exit.json)。
