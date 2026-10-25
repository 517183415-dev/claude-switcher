# A03 用户入口独立复核

复核者：本地只读子代理 `f01_review`。结论：该入口组未发现剩余 P1/P2。

1. 四个维护命令持有 Publisher 锁，并在同一个 LocalObjectStore publication 锁内认证两份 current。维护调用链不访问 Keychain。
2. Personal 注册并授权四个命令。Public 不注册、不授权这些命令。
3. 预览展示保留及移除的历史、素材保护原因、候选文件和 GC 分类。索引整理与文件回收分别确认；取消不执行，忙状态不重复执行，过期预览由后端拒绝。
4. A01 容量限制和 A02 发布反馈合同保留。fixture 不触发真实 IPC。
5. 已核对前端 60 项方法、2 组 App VM、Svelte 零错误零警告和两版构建回执。复核者未重跑测试。

本结论只覆盖入口组静态复核。核心文件另行复核。真实 IPC、GUI、安装、账号和实际系统行为均未运行。

| 文件 | SHA256 |
| --- | --- |
| apps/desktop/src-tauri/src/publisher_commands.rs | 2837b5311734f3083bff772baaa52d92055bd9a7d2c83b67a103f726d98420bb |
| apps/desktop/src/pages/Operations.svelte | 8476dd4e35d5550e4069a6d0dc9e76ef91db25ebdf680c1f5e81e1968f4a976a |
| apps/desktop/src-tauri/src/main.rs | 5a0c4b218bd3f425231431e6805a3293efb05a03d87a2ec2f05ac172354311f0 |
| apps/desktop/src-tauri/build.rs | 33f92d6763de81c1c7368d51b6726f9117501305a41e07cbfb39905f4036df2f |
| packaging/editions/personal.json | 76a96b880c0828a5e0aee1da902857aec37a8a4a7f69a7c5446df1846db3590c |
| apps/desktop/tests/publisher-maintenance.test.mjs | 661c2d9cfd33931db1e581a86c5cf750c9ede6d04bee1fea87804db780fcfb4c |
