# 发布器与接收端组件的本机联通

## 范围

现有集成测试使用合成密钥、预览配置和回环模拟对象存储。正式发布流程调用 AWS S3 SDK，上传两版签名清单和图片。接收端 `ContentSource` 生成请求路径，`AdCache` 校验并采用两个版本；图片通过接收端校验。测试从模拟源返回旧版和被篡改的签名数据，确认拒绝且保留第二版。

测试只把固定来源的路径映射到本机 HTTP。它不运行生产 `HttpsFetcher`、桌面后台定时器、真实 Cloudflare、TLS/CDN 或线上传播。既有受控 HTTPS 和调度逻辑测试保持独立证据，不把此项称为端到端线上验收。

## 结果

- `configured_publication_uses_sdk_upload_readback_and_conditional_updates`：1 项通过、0 项失败。见 [test.log](test.log)。这是扩充已有测试，不是额外累加的独立测试。
- Personal 桌面测试目标 Clippy 通过。3 条既有 `migration.rs` 风格警告保留。见 [clippy.log](clippy.log)。
- 修改的 Rust 文件格式检查通过；CodeGraph complete、pending 0。见 [graph-status.json](graph-status.json)。
- Rust 测试日志明确记录 `test result: ok`。外层命令在测试结束后把退出码赋给 zsh 的只读变量 `status`，因此该外层命令退出 1。测试未重跑；日志由独立解析核对通过。

源码摘要见 [inputs.sha256](inputs.sha256)。本次没有修改产品逻辑或依赖。
