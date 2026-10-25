R08 冻结源码独立复核：原 P2 已关闭；本轮窄修复未发现剩余 blocker/P1/P2。

两处 regex 支持 header/type 合法尾注释、成对引号和 CRLF；select#suffix 与 url-test # select 不被误认。新增3方法覆盖 main 在 locks/stopped/transact 前拒绝歧义、Second 精确接受、Missing 拒绝。原无注释/嵌套列表矩阵保留。

当前源码 SHA 与 final-runtime.json 一致。旧生产 hash 下的 type/header RED 与新冻结源 18/18 GREEN 相符。其余配置、digest、提交并释放锁后的 owner GUI 交接和保护合同延续第一轮结论。

审查者只读，未修改文件、未重复测试、未运行真实操作。最终全量178/178、whitespace及30函数/14文件保护由根会话统一guard另行收证。没有可用LSP/ast_grep工具，不声称其通过。先前工具链下载边界异常仍保留。
