R09 本地执行边界

用户已停止使用 codex-with-chatgpt skill。禁止 C2C CLI/MCP、浏览器会话及外部助手协作。保留此前已完成审查为历史证据。

只改已批准 helper 最小范围，先隔离行为回归。不得安装、下载或使用真实账号、产品网络、GUI、native IPC、服务或系统设置。Rust 验证必须使用已存在的 1.96.0 工具链直接二进制，固定缓存，CARGO_NET_OFFLINE=true，不经 rustup 代理。

R08 的历史工具链下载边界异常继续保留，后续GREEN不能覆盖它。
