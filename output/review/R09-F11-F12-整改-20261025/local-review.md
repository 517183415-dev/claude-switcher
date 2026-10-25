# R09 本地独立审查

审查者：本机 Codex 原生子代理 /root/f01_review。未使用 codex-with-chatgpt、外部 ChatGPT 或浏览器。

结论：未发现剩余 blocker/P1/P2。审查针对 platform.rs、policy.rs、main.rs 最终源码。

F11 三个写点使用同目录随机 create_new、0600、NOFOLLOW/CLOEXEC。写入和同步后核验描述符与路径的设备、inode、uid/gid、长度、普通文件及单链接。journal 保留任意已有节点，并用 RENAME_EXCL 防止发布竞态。失败不清理未知残留。component/managed 保留原替换语义。

F12 严格校验 journal、ID、字段、私有 results/receipt、完整 plan、code 与 owner。只有真正缺少 receipt 才允许 expires_at=0 的旧内部恢复，且须精确匹配 root 私有 managed 备份。坏 receipt 不回退。内部 writer 保持不变。status 读取真实状态。启动仅放行五项明确的恢复业务冲突；信任、未知、IO、cleanup、system_command 错误仍终止启动。main 保留 SourceLocal 与 RootLock 顺序。

policy 在 expired Apply 的写操作之前施加 pending 门槛。允许的请求继续经过原 session、sequence、role 和 grant 校验。清除 journal 后动态恢复正常入口。xpc.rs 与 root_operation.rs 字节未变。

测试可以支撑本次窄改。启动分类测试没有执行 main/listener。真实启动、XPC、授权和系统设置保持 NOT_RUN。LSP/ast_grep 工具未提供，未声称通过。
