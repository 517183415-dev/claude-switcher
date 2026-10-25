R08 独立只读审查：REQUEST CHANGES

冻结源码：local_install.py 3c1c250151535ed0fb9b6ebe601b5fb2e7e222930b3a67d122d684e0c1ad81b8；test_local_install.py e30f3edd23ee0764eeb3e58e5c9214b9e88a4ee5bf3b6418d430b3d261f6ff97。

P2：local_install.py:1159,1165,1171-1176。带合法尾注释的 proxy-groups header 或 plain/single/double quoted select type 被遗漏。First 普通 select 与 Second 带注释 select 会被误判为唯一组；带注释 header 可使错误显式组名按零候选接受。

要求：仅补齐现有 block-style 合同的合法注释、匹配引号及 CRLF；两组无选择在 locks/stopped/transact 前拒绝；Second 精确接受，Missing 拒绝。补真实 RED 后最小修复。

其余配置选择、显式配置零探测、digest、owner/edition/generation、锁外普通GUI交接与组件/环境语义未见其它具体 blocker/P1/P2。30个保护函数及其余14个文件未变化。审查者未改文件、未重复测试、未运行真实系统操作。

另有首次全模块验证环境边界异常，详见 f07-boundary-violation.json。该轮 NOT_ACCEPTED，不能计作通过或无下载证据。
