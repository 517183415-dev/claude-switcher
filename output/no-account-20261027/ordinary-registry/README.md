# 普通 IPC 共享注册表与业务分发

普通入口和 MockRuntime 现在共用 `ipc::handler`，统一 23 个公共命令和 25 个 Personal 运营命令。原 Personal/Public 的 48/23 个有序名称保持，见 [registry-preservation.json](registry-preservation.json)；独立与实际生成的 ACL 对照，不仅核对名称数量。

`launch_checked`、`account_action` 和共用退出函数只将 AppHandle 参数泛化为 Runtime，不改变业务、成功条件或退出时序。特殊 ipc-smoke/ipc-business/ipc-limits 入口继续独立；未新增产品测试命令或业务替身。

测试在 Tauri MockRuntime 中执行真实 InvokeRequest JSON 解码、现行普通 ACL 和完整产品分发表。覆盖两版全部注册命令的其他窗口/两类远程来源拒绝；Public 25 个运营命令拒绝；公共 9 个及 Personal 另 3 个命令的缺参拒绝。正控制读取 runtime_info，拒绝矩阵结束后独立确认合成根目录无业务文件。

两版还通过真实处理函数保存非默认偏好，拒绝未确认广告用途及未知字段且保持已提交字节，读取合成敏感活动并导出，独立核对固定路径、返回值与磁盘 JSON 一致、敏感标记消失、文件 0600。重建 App/DesktopState 后回读及再导出保持。Personal 既有三槽六期草稿测试也改用完整产品分发表。

首轮 Public 编译失败来自测试辅助方法 persisted 缺少 Personal 条件，原始失败保留 `targeted-public.log/json`；修正后 `targeted-public-r2` 通过。这是测试条件编译问题，未当作产品业务缺陷。定向 Personal 6 项、Public 4 项；最终完整结果另见 final-checks.json，重复执行不累计为不同测试名。

Mac 本轮仍锁屏，未尝试原生窗口。MockRuntime 与生产分发表共用不等于 WKWebView、所有业务成功分支、系统进程重启、原生 Keychain 或真实存储验收。只使用合成目录、隔离 HOME 和 account-free 沙箱。注册、部署、付款保持暂停；SwiftUI 流体玻璃 UI 最后执行，保留当前布局。

最终集中验证：Personal 230 项、Public 35 项全部通过，0 失败/忽略。两版 Clippy 和变更 Rust 格式检查通过；Personal 保留三条既有迁移风格警告，仍有既有 ts-rs 属性提示。源码绑定未变化。见 [final-checks.json](final-checks.json) 与 [receipt.json](receipt.json)。前端未改，复用上一阶段前端检查，不重复执行。
