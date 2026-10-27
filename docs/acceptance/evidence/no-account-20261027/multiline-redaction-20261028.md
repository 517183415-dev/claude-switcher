# 活动记录的多行敏感值

原有规则只隐藏包含敏感字段名的一行。若字段值位于后续行，敏感值可能进入活动记录和导出文件。合成 Authorization 续行回归先复现此问题：[修复前失败](../../../../output/no-account-20261027/multiline-redaction-20261028.CLebaw/red-core.log)。

现在，共享脱敏函数遇到既有敏感字段名时，保留此前已脱敏的内容，并隐藏该行和整条消息的后续内容。隐藏余下内容是为了处理长度未知的多行字段和嵌套对象；因此后续普通信息也会被隐藏。规则仍依赖现有字段名识别，不能保证识别任意未标记或变形的敏感数据。

1. 最终核心脱敏测试 2 项通过，包括多行、嵌套、多个字段和幂等性。[日志](../../../../output/no-account-20261027/multiline-redaction-20261028.CLebaw/core-final.log)。此前同一业务实现的持久化/预览/导出序列化契约也通过。[日志](../../../../output/no-account-20261027/multiline-redaction-20261028.CLebaw/core.log)。
2. 两种发行特性分别运行新增的桌面测试，均通过；测试检查合成敏感值、嵌套结构、新记录保存、预览、固定导出文件、历史记录回读和后续独立事件。[Personal](../../../../output/no-account-20261027/multiline-redaction-20261028.CLebaw/desktop-final.log)、[Public](../../../../output/no-account-20261027/multiline-redaction-20261028.CLebaw/public-final.log)。
3. 原有桌面日志回归通过；核心库严格 Clippy 通过。[回归](../../../../output/no-account-20261027/multiline-redaction-20261028.CLebaw/existing-roundtrip.log)、[Clippy](../../../../output/no-account-20261027/multiline-redaction-20261028.CLebaw/clippy.log)。编译输出包含既有 ts-rs 属性解析提示。

测试使用合成目录，禁止网络和默认 securityd。历史原始文件在读取和导出时进行脱敏，但不会因为只读操作立即改写；下一次成功追加活动记录时才会替换旧文件。未对真实历史数据执行清理，未验证原生窗口、真实账号或真实云服务。本轮尚未重建产品包。
