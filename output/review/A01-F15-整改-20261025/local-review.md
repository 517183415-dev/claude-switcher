# A01 / F15 本地独立复核

审查者：本地f01_review；root另核diff、六路径冻结哈希及回执，并重新执行真实源函数UI测试5/5。

未发现blocker/P1/P2。真实async命令进入生产共用restore body，再进入原Publisher.restore_backup。64MiB inclusive gate复用格式常量；图片命令复用1MiB核心常量，UI文案及门槛一致。backup.rs和publisher/mod.rs与基线字节相同，原atomic generation路径不变。

已核Rust4/4、UI5/5、既有备份1/1，共10个独立方法；clippy/fmt/diff exit0。真实大密文使用合法JSON尾空白padding，不冒充素材负载。源/目标active、generation、draft及MemoryVault失败原子性有断言；晚期失败留下未引用generation属既有行为且明示。

未覆盖Tauri反序列化、spawn_blocking调度、64MiB WebView传输、Keychain或GUI。首次rustfmt未覆写HOME的环境记录保留，不宣称该次完全隔离。没有C2C审查调用。
