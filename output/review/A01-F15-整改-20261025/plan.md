# A01/F15 最小计划

1. 保留旧 32 MiB gate，抽取命令实际 blocking body；production 与测试共用。测试 callback 仅注入临时 Publisher 与 MemoryVault。
2. 用现有 export 生成合法备份，解密后加入合法 JSON 尾空白并以原 KDF/格式 seal。33 MiB 与精确 64 MiB 文件经共享 body 恢复。padding 不代表大量真实素材。
3. 先捕获 33 MiB body 恢复失败及真实 Svelte imageInput 超 1 MiB 仍 invoke 的行为 RED。编译/夹具失败另记。
4. 命令复用 backup::LIMIT 与 ads::MAX_IMAGE_BYTES；UI 限制 1 MiB，备份提示“不超过 64 MiB”。格式/KDF/mod.rs 保持不变。
5. 仅离线定向 Rust 和实际 UI handler/mock invoke；覆盖边界、口令/篡改、失败原子性。真实 Tauri IPC/WebView、Keychain、GUI、系统、网络均 NOT_RUN。

写范围：publisher_commands.rs、publisher/tests.rs、Operations.svelte、新 publisher-limits.test.mjs。其余源码只读。
