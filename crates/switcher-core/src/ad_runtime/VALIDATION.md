# 广告客户端本地验证证据

日期：2026-10-22。

执行：`RUSTUP_TOOLCHAIN=stable cargo test --locked -p switcher-core ad_runtime::`。
结果：10 passed，0 failed。测试耗时 35.68 秒。

1. `configured_pins_with_all_slots_off_make_zero_requests` 配置有效测试 pins 和 MemoryFetch 内容。
   用户已确认请求用途，但三个广告位全部关闭。请求记录为空，最高序号仍为零。
2. `controlled_https_ca_signed_manifest_assets_privacy_and_negative_cases` 使用真实 loopback TLS。
   独立 CA:TRUE 根证书签发带 DNS SAN、serverAuth 和 CA:FALSE 的叶证书。
   客户端只增加该测试根证书，固定域名解析到 loopback。清单与 PNG 完成下载、签名与素材校验并可展示。
3. 同一 HTTPS 测试拒绝未信任 CA、HTTP 重定向、签名篡改与超限响应。
   捕获的两次成功请求只有固定 production 路径，没有 query、认证、Cookie、Referer、User-Agent 或账号/设备标识头。
4. 其余测试验证共享时钟恢复、离线期限、单广告位素材门控、调度持久化、失败素材保留最高序号、商家确认和缓存防重放。

测试不使用 `danger_accept_invalid_certs` 或禁用主机名校验。
测试证书、签名密钥和 DNS 覆盖只存在于 `#[cfg(test)]` 模块。
测试使用本机 OpenSSL 和 Python 生成合成 TLS 服务。服务在 fixture 析构时终止，临时目录随后删除。

生产 `ContentPins::production()` 仍为 `None`。
生产零请求等待不能替代已配置源的全关证明；两条路径有独立测试。
此证据不覆盖真实托管、真实生产证书链、真实 Keychain、远端 CAS 和线上持续运行验收。
实际单服务适配器所需材料见 Personal 发布器的 `publisher/README.md`。
