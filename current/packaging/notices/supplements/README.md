# 精确版本的补充许可材料

本目录补充 macOS arm64 图中原先缺少的 16 个 Cargo 包和 4 个 npm 包。
条款采集和法律审查是不同状态。最终二进制链接 SBOM 未在本次工作中生成。

## 已取得完整项目许可证或明确引用许可的 7 项

| 包 | 精确版本绑定 | 补充材料与适用依据 |
|---|---|---|
| alloc-stdlib 0.3.0 | 锁定 `.crate` 内 VCS 提交 `0a81fd6928ea3b33c8cd484aa4575d50ffb98012` | 同提交主项目根 BSD-3-Clause LICENSE，适用于 `alloc-stdlib` 成员 |
| selectors 0.38.0 | 锁定 `.crate` 内 VCS 提交 `572ecba2d1600e7c3d490586692a209faf703baa` | 保留包内源码 MPL-2.0 通知，并收集其明确链接的 Mozilla 官方完整条款 |
| tauri-plugin 2.7.2 | 锁定 `.crate` 内 VCS 提交 `59b63ab500da9859f12d8888c3298bc29bba0b92` | 同提交主项目根 MIT/Apache-2.0 全文，适用于 `crates/tauri-plugin` 成员 |
| ts-rs 12.0.1 | 锁定 `.crate` 内 VCS 提交 `7182ad8289596097235406b715fa04506443b4ad` | 同提交主项目根 MIT LICENSE，适用于 `ts-rs` 成员 |
| ts-rs-macros 12.0.1 | 同上 | 同提交根 MIT LICENSE，适用于 `macros` 成员 |
| @rolldown/binding-darwin-arm64 1.2.15 | 同版本 `rolldown` 主包 tarball 对锁定 SRI 校验通过 | 完整主包 LICENSE 与 THIRD-PARTY-LICENSE；主包精确依赖此绑定，两个包声明同一仓库与 MIT 标识 |
| @tauri-apps/cli-darwin-arm64 2.12.4 | 同版本 `@tauri-apps/cli` 主包 tarball 对锁定 SRI 校验通过 | 完整主包 MIT/Apache-2.0 全文；主包精确依赖此绑定，两个包声明同一仓库与许可标识 |

同版本主包共享文本不构成对平台绑定内部链接代码的独立 SBOM 分析。
主包已附带的第三方通知保持原文，不代表内部第三方覆盖已被单独审定。

## 仍未关闭的 13 项上游专属通知缺口

1. 11 个 objc2 系列包的精确提交只有根 `LICENSE.md`，其中声明 MIT 或 Zlib/Apache-2.0/MIT，并链接标准条款。
2. `is-reference` 3.0.3 的精确提交 `8bb053129bfabe2f6a7d7ed050159d67ebe82829` 只有 README 的 MIT 声明，没有 LICENSE 文件。
3. `locate-character` 3.0.0 的精确提交 `4f08a59ec248121f7002abd02ee7b94e8eda06bc` 同样只有 README 的 MIT 声明，没有 LICENSE 文件。

两个 npm 提交来自官方 registry 的版本 `gitHead`；registry integrity 与锁文件一致。
`locate-character` 的 registry 仓库字段仍指向 GitLab；当前官方 GitHub 仓库有相同完整提交，且提交内 package.json 的名称、版本、MIT 声明一致。

本目录保留这些原始声明，并附 SPDX 官方 license-list-data v3.28.0 的标准全文。
标准全文仍含原始版权占位符。没有推测版权人，也没有用作者字段填充占位符。
因此它们是“条款与声明已归档、项目专属通知待核”，不能称作“完整通知缺口已经关闭”。

objc2 的精确提交声明还包含 Apple SDK 衍生作品适用性说明。
该说明在本目录保持原文。此工作没有解决或裁定该法律适用性问题。

## 证据链与复现

```text
Cargo.lock checksum -> 本地 .crate -> VCS commit/path -> 同提交声明或根 LICENSE
npm lock integrity  -> registry gitHead / 同版本主包 -> 声明或共享 LICENSE
上述原文 + 官方标准条款 -> sources.json SHA256 -> 离线生成 inventory 与子集报告
```

- [sources.json](sources.json) 逐项记录文件 URL、SHA256、精确版本、提交、适用依据与未关闭项。
- Cargo `.crate` 完整 SHA256 对锁文件校验。版本与 VCS 信息直接来自该归档。
- npm 主包 tarball 完整 SRI 对锁文件校验。registry 证据仅保存公开必要字段。
- 正常复现：先执行 `python3 packaging/notices/generate.py`，再执行 `python3 packaging/notices/macos_subset.py`。两者不联网。
- 单独重新采集官方原文：`python3 packaging/notices/fetch_supplements.py`。此命令联网，但不安装包、不修改锁文件。随后离线生成器校验已归档原文的 SHA256。

普通 AnySearch 搜索只用于定位官方来源。搜索结果不承担版本绑定或许可原文证明。
原文来自具体提交、官方 registry 发布包、Mozilla 或 SPDX 官方规范源。
没有使用博客中的通用许可文本，也没有执行动态页面全文抽取。
