# 普通恢复 IPC 拒绝路径

新增一个测试名，使用普通产品共享注册表、实际 Tauri 参数解析、当前 Personal ACL 和生产异步 handler。两个命令分别为 `restore_publisher_backup` 和 `publisher_archive_import`。没有增加产品测试入口或替换 Vault。

每个命令测试空容器、错误 magic、截断、不支持 KDF 参数、错误口令、过短口令、认证标签篡改和认证有效但不是快照的 JSON，共 16 条响应，不称为 16 个测试名。生产 seal/open 对合成明文完成正控制。生产图片导入与暂停先建立含素材的非空发布状态。

每次拒绝均校验具体错误；递归比较整个 publisher 子树的路径、文件/目录类型、权限与文件字节。最后销毁并重建 App 状态，草稿及子树保持一致。不是操作系统进程重启。快照不覆盖 mtime/xattr，亦不对外推断真实 Keychain 状态。

阶段末相关 `ipc_` 集合 7 项通过；Personal all-target Clippy 与格式检查通过。保留三条既有迁移风格警告及 ts-rs 提示。仅新增测试和私有测试快照函数，产品业务未修改，因此不重复 Public、前端和桌面全量测试。没有发现新产品 bug，无失败修复记录。

命令和哈希见同名 JSON；293 个输入的绑定见 `inputs.json`，运行前后全部一致。隔离使用 `../storage-directory-race/account-free.sb`，复用 `../ordinary-registry/home` 和 `tmp`，Cargo offline/locked。真实账号目录及外网仍被沙箱阻止。

成功备份导出/恢复 handler、真实 Keychain、原生 WKWebView 和真实远程传播仍未证明。注册、部署、付款暂停；本轮不尝试窗口、不启动子代理。
