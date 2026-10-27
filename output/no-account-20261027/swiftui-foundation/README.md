# SwiftUI 基础阶段证据

状态：PARTIAL。产品入口仍是现有 Svelte/Tauri；没有切换到 SwiftUI。

- [当前布局基线](../../playwright/swiftui-layout/)：六张截图使用合成数据。
- [总览原生预览](overview-v4.png)：macOS 27，合成数据。底部广告区域在截图折叠下方，未完成视觉核对。
- [视觉评估](visual-verdict-v4.json)：86 / revise。其他页面与产品接线未完成。
- [Swift 测试日志](swift-test-ui.log)：5 项桥接测试通过。该日志先于最后一轮仅视图修改；[最终视图编译](swift-build-v4.log)通过。没有重复运行既有测试。
- [隔离真实 WebView 桥接回执](../swiftui-native-bridge/r2/result.json)：固定命令调用、错误与未知命令拒绝通过。Public 检查仅为 Swift 侧门禁，底层窗口仍是 Personal 测试夹具；不是 Public 实窗验收。

测试使用合成 HOME 和密钥库，不访问真实账号、不连外网。尚未完成真实数据及操作绑定、原生宿主、全部页面、产品打包、macOS 13–25 或真实账号验证。注册、部署、付款均未进行。
