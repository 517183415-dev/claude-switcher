# 普通命令分发：草稿业务闭环

范围：Tauri MockRuntime。实际使用 Tauri InvokeRequest 的 JSON 参数解析、现行普通权限解析器，以及六个生产 publisher 命令处理函数。测试不启用 ipc-business/ipc-limits/ipc-smoke，不把模拟运行时称作真实 WKWebView。

## 覆盖内容

1. 导入合成 PNG，实际规范化并写入私有素材目录；预览内容与导入返回一致。
2. 三槽六期草稿保存、独立读取磁盘 state、重建 DesktopState/App 后回读。逐槽修改第二期，检查其他槽保持原值；缺参和重叠排期拒绝且已提交 state 字节不变。
3. 逐槽暂停与全部暂停；检查草稿持久值与未配置签名密钥的真实状态。
4. 其他窗口对六个命令均须返回 ACL not allowed。损坏状态下读取、状态查询及暂停均须失败，原始损坏字节不得被初始草稿覆盖。

两个新测试首次定向通过，见[首次回执](tests-r1.json)。最终桌面后端套件 228 项通过，0 失败；Clippy 与格式检查通过，Clippy 保留三条既有迁移风格警告。最终集中检查和源码绑定见[最终回执](final-checks.json)与 [final-inputs.json](final-inputs.json)。初次定向与最终套件中的重复执行不累加为新测试名。

测试共用已有权限解析器与隔离 DesktopState 构造器。仅在 dev-dependencies 启用既有 Tauri 的 `test` 功能；锁定版本中该功能为空，不增加依赖库，Cargo.lock 未变。测试路由仅注册六个受审命令，不代替完整产品命令注册表验收。

Mac 会话通过 CGSessionCopyCurrentDictionary 的 CGSSessionScreenIsLocked 字段确认仍锁屏，本轮未重试窗口测试。真实 WebView、整个原产品前端、完整业务 IPC、原生 Keychain、真实存储与系统托管 WebKit 回收仍未验证。只使用隔离 HOME/临时目录和 account-free 沙箱；注册、部署、付款未执行。UI 重写仍排最后，保留当前布局。

Public 测试目标编译检查通过，见 [public-check.json](public-check.json)。该检查未运行 Public 测试或窗口。最终汇总和哈希核对见 [receipt.json](receipt.json)。

UI 最后实施：SwiftUI 流体玻璃质感；当前布局、导航和功能位置保持；不复制参考图布局，不使用蓝紫色背景。
