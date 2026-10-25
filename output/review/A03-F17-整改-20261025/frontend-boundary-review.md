# 前端验证边界只读复核

结论：当前8个 tests/*.test.mjs 与 core-preparation-regression.mjs 可在合成镜像运行。现有测试使用源码读取、VM、合成数据和fake IPC，未发现产品原生、真实账号、网络、浏览器或系统设置调用。

recovery-copy 和 repair-preview 会在镜像tests目录写入并删除临时模块。时区测试只修改当前Node进程TZ并还原。构建工具会运行普通shell/Node/esbuild子进程，并写入镜像dist/cache，不能称零子进程。

运行前清除 APP_SOURCE_PATH、REPAIR_BASELINE、REPAIR_REVISION2_BASELINE、NODE_OPTIONS；合成HOME、TMPDIR和npm cache；缓存依赖只读复用；npm --offline test/check/build。无pre/post生命周期hook。无需加入专门无障碍验收。

审查者：本地只读 agent /root/audit_runtime。未运行测试。本记录为静态边界判断，实际结果另见frontend-final-receipts.json。
