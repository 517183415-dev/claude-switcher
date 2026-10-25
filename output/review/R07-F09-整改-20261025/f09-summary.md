# R07 / F09 最小一键修复切片

## 已修改并冻结

仅 RegionSettings.svelte、App.svelte、repair-preview.test.mjs。没有修改 Rust/Python、queue/recovery 测试、依赖、CSS、布局或颜色。

- 普通环境按钮通过实际 markup 的 fix handler 自动 preview_repair → execute_repair，同一 requestId 只消费一次。
- 地区选择仍用 plan 纯预览。jp 用例确认 region=jp，显式 execute 使用同一 requestId。
- 父级同步 acquire 使用 requestId 所有权，拒绝 busy 或已有 repair owner。旧 completion 只能释放自己的 owner。
- 开始修复时推进 checkEpoch，立即清空旧报告并抑制 snapshot 恢复旧证据。只有 completed=true 才释放父修复锁并复用 runChecks 执行一次完整检查。
- cancel/destroy 失效组件 epoch。已派发 execute/recover 的父锁保留到当前 promise / readback 结束；迟到 completion 不发 outcome、不 load、不 full check。
- recovery 完成后 load → plan 仍为纯预览，0 自动 execute，0 full check。

## 证据

`f09-red.log` + `f09-red-manifest.json` + `f09-red-sources/`：18 项，8 pass、10 行为 fail，退出码 1。入口取自实际按钮 markup；没有缺失函数 ReferenceError 或依赖问题。

`f09-green.log` + `f09-green-manifest.json`：21 项全部通过，退出码 0。`f09-green-receipts.json` 含 18 个真实生产 VM 场景的调用序列/计数、parent busy、报告、outcome、error。普通成功及双阶段连点均为 preview/execute/check=1/1/1。blocked、recovery_required、preview error、cancel preview 为1/0/0；execute false/error/cancel/destroy 为1/1/0。地区显式执行为 region preview/execute/check=1/1/1；recovery 为 preview2/execute0/check0。fixture 为0 IPC。

测试在 offline-runtime.json 的既有离线镜像中执行，使用复制好的精确依赖及合成 HOME。每次运行前同步3个拥有文件，未安装、下载或调用真实 Tauri IPC。App 和 RegionSettings 的生产 script 使用 TypeScript 转译后在 VM 中执行；子组件 callback 取自生产 App markup 属性表达式。VM 不模拟 Svelte DOM 响应式刷新；同步父 acquire guard 的实际函数覆盖跨组件互斥。

`f09-diff-check.json` 退出码0；`f09-frozen.json` 含最终 hash/行号。早期 GREEN attempt1 作为历史保留。

## 未验证边界与交接

本切片未运行真实 GUI、native helper 授权、账号、网络、应用、服务或系统修改。目标测试包含生产预览 markup 的 SSR 渲染，但不声称真实浏览器点击/交互验收。完整 npm test、Svelte check、两版 build、queue/recovery 以及13个已接受文件的不漂移核对，由父会话统一执行。保留现有恢复文案。
