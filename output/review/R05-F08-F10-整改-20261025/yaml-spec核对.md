# YAML tab 与注释分隔核对

本机 PyYAML 6.0.3 拒绝两个原始 tab header，也拒绝保留同样 tab 的 Rust 输出。独立 Psych 3.1.0 接受相同原文与输出。完整文本和 SHA256 位于 `yaml-parser-psych-report.json`，PyYAML 失败仍保存在 `yaml-parser-report.json`。

通过 AnySearch 定位的官方来源：[YAML 1.2.2 规范](https://yaml.org/spec/1.2.2/)，发布于 2021-10-01。规范的 separation spaces 规则允许行内 token 分隔包含 tab。comment 规则要求注释与其他 token 之间有空白。这与保留 `prepend-rules:\t`、拒绝无分隔的 `prepend-rules:#comment` 的合同一致。

AnySearch 请求记录：普通搜索 `05f1d3dc-e198-4985-9753-2f191d744243`，行内分隔查询 `8ef852cb-6925-4b8f-8125-aa3d26557d97`，注释分隔查询 `d106f073-6b10-4dc4-99b7-161370de9158`。`list_domains --domain code` 返回 tool not found，纵向格式元数据不可用；后续使用普通官方域名搜索。未使用 Firecrawl 或启动相关服务。

本证据核对语法和解析器差异。真实 Clash 加载验收保持 `NOT_RUN`。没有安装依赖，没有规范化或改写 Rust 实际产物来消除解析器分歧。
