# R05 / F10 实现与证据

状态：F10 源码已冻结。本片仅修改 `crates/switcher-core/src/pinned_routes.rs`。父会话负责统一模块回归、Clippy、格式检查和累计冻结文件核对；独立代理负责已有 PyYAML 校验。

## 根因及修复

旧 `propose` 只识别字节完全等于 `prepend-rules:` 的行。合法尾空白或注释会被漏认，导致新增重复顶层键。无目标键时，旧实现把新块插到整个文件前面，可能落在 `---` 之前。

新增小型顶层行分类器。分类器同时用于原文的文档边界检查和清理后的目标定位。它接受空目标值及空白分隔的注释，拒绝 inline 值和无法确定的顶层键语法。冒号后直接接 `#` 不属于合法空块头，明确拒绝。

清理管理块后只允许一个顶层目标键。存在唯一开头 `---` 时，在该 header 后插入；存在目标键时，在其原始 header 后插入。原始行不重排、不规范化，生成规则沿用换行风格。管理块旧格式迁移保留。`first_select_group` 的源码未修改，未增加依赖。

## 行为 RED → GREEN

最终命令：

```sh
cargo test --offline --locked -p switcher-core --lib f10_ -- --nocapture --test-threads=1
```

| 阶段 | 结果 | 文件 |
|---|---|---|
| 首轮行为 RED | 0 passed / 6 failed，exit 101 | f10-red-initial.log / manifest / source |
| 补充过程的编译失败 | NOT_RED：并行 macos.rs 的日志格式错误，测试未执行 | f10-red-compile-failure.log / manifest |
| 最终行为 RED | 0 passed / 7 failed，exit 101 | f10-red.log / f10-red-manifest.json / f10-red-source.rs |
| GREEN | 7 passed / 0 failed，exit 0 | f10-green.log / f10-green-manifest.json |

真实失败断言包括：尾空白导致重复顶层键、document header 前插入、nested 后真实目标头识别错误、inline 值被接受、duplicate 被接受、缺少 YAML 注释分隔符被接受、旧块迁移后注释目标头被漏认。

## 实际产物矩阵

`f10-fixtures.json` 从 GREEN 日志里的 `F10_FIXTURE` JSON 行提取。所有 output 和 second_output 均为 Rust 实际 `propose` 返回值；没有使用 Python 重新构造结果。

| 分类 | 子例数 | 结果 |
|---|---:|---|
| plain / 空格 / tab / 注释 / CRLF 注释 / 无结尾换行 | 6 | accept；原字节和插入点相符，二次不变 |
| 单 document header、前置注释空行、CRLF、无结尾换行 | 4 | accept；规则位于 header 后，二次不变 |
| nested-only、nested-before-target，含 scalar 内伪键和 marker | 2 | accept；嵌套用户字节不变 |
| 旧顶层管理块迁移，含 anchor 与用户目标头/规则 | 1 | accept；只有一个目标头，用户内容不变 |
| inline 值 | 10 | reject；output=null |
| duplicate、文档歧义、复杂键语法 | 13 | reject；output=null |
| 不完整、嵌套、歧义管理标记 | 7 | reject；output=null |
| 冒号后无空白的 #comment | 1 | reject；output=null |

合计 44 个子例：13 accept / 31 reject。所有 13 accept 包含 actual second_output、second_changed=false、group；每份输出含 13 条管理规则。

## 冻结与边界

冻结 SHA-256：`2981976a310b98af41493751b32f12d8008b53a5b71276f13ba0cd72fa37aa87`。GREEN 前后哈希相同。已执行本文件 rustfmt；本文件 git diff --check 返回 0。未修改其他 owner 源码。

这是严格顶层文本子集，不是通用 YAML parser。复杂顶层 key、directive、document end、inline target value 保守拒绝。原有普通用户文本、nested 数据、注释和 anchor 在接受路径保留。既有测试中原来的 `ends_with(complex)` 错误位置断言改为同时检查 `---` 在前和原 profile/anchor 字节位于尾部。

没有调用真实应用、代理、网络、账号、服务或系统设置；没有安装依赖。独立 YAML parser 和真实 Clash 接受情况不能由本片 Rust 测试直接推断；独立 parser 结果由另一个代理提供，真实 Clash NOT_RUN。
