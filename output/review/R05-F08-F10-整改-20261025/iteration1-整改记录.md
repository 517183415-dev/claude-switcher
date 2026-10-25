# R05 F08/F10 第一轮本地结果

状态：本地隔离验证与 ChatGPT 复核通过；R05 无剩余项。任务 `c2c_824e`，iteration 1。

## 最小修复

F08 仅将命令观察时限参数化。通用命令仍固定两秒，DNS 使用四秒。原固定 dig 参数、首行选择、环境、输出上限、编码及状态检查保持。timeout 继续显式 kill 和 wait，取消保留 kill_on_drop。测试仅创建本机假 Python 子进程，不导入 socket 或调用真实 dig。

F10 使用同一个严格行分类结果识别和插入顶层 `prepend-rules`。接受合法尾空白/分隔注释、CRLF 和单文档头。新块放在文档头之后。inline、重复键、多文档、歧义键拒绝。保留用户原文、nested 内容和旧管理块迁移。`first_select_group` 完全未改，没有增加 YAML 依赖。

## 行为 RED 与最终验证

- F08 有效 RED 为 4 方法，3 过、1 失败：2.5 秒假答案在约两秒被截断。GREEN 四方法通过，覆盖十个真实假子进程案例。两种超时均留下 kill_ok/wait_ok 回执。DNS 成功约 2.54 秒；通用命令仍约两秒超时。
- F10 有效 RED 七方法均行为失败；GREEN 七方法通过。真实 Rust 输入/输出矩阵为 13 接受、31 拒绝。编译失败及夹具碰撞另行保留，未计入行为 RED。
- 最终九条命令通过。互不重叠模块为 macos 11、user_repairs 41、regions::legacy 15，共 67 方法。F08=4、F10=7、pinned_routes=9 已含于模块数。
- 离线 Clippy、两个文件的 rustfmt 和差异检查通过。每条命令前后核对 11 个累计修改文件哈希，R01–R04 九文件未变。
- 独立只读 reviewer 未发现可操作阻塞，未写源码或重复测试。

完整命令、退出码、时间、测试数及源码哈希见 `verification.json` 与 `final-receipts.json`。

## 独立 YAML 结果

验证只消费 `f10-fixtures.json` 的真实 Rust 输出，没有重建预期输出。

PyYAML 6.0.3 返回 42 PASS、2 FAIL，退出 1。两个失败均为保留的 tab header，原始输入也失败。现有 Psych 3.1.0 接受这两份原文及输出，并验证每例 13 条规则、管理优先、用户 bytes/nested 语义、changed 和实际二次幂等。13 接受案例均获得独立验证，31 拒绝案例都没有 proposal。

PyYAML 失败未删除或改为通过。初版 validator 的六个边界修正保留在说明中。官方 YAML 1.2.2 规则与解析器差异见 `yaml-spec核对.md`、`yaml-parser-notes.md` 和各命令回执。

真实 Clash 加载、真实 DNS/外网、安装、真实账号、应用和系统设置/服务均为 `NOT_RUN`。长期验收为 `WAIT`。没有安装依赖、提交、推送或合并。
