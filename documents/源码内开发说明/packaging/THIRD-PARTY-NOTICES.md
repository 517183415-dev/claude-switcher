# 第三方依赖与许可声明材料

本材料绑定当前锁文件。它是离线生成的 **overinclusive 锁全集**，不是最终二进制 SBOM，也不是法律审查通过证明。
生成器只读取项目锁文件、项目 node_modules、本机 crates.io registry 缓存和已归档的 supplements。生成器不联网、不安装或升级依赖。补充材料曾从官方发布源与精确提交采集。

## 范围与打包边界

1. Cargo 表包含根 Cargo.lock 的全部第三方版本。排除四个第一方 workspace 包，以及 experiments 的独立锁文件。
2. Cargo.lock 不区分运行时、proc-macro、构建工具、平台与 feature。此表保留所有目标条目，不能断言它们全部进入 macOS 工件。
3. npm 表保留 package-lock.json 的全部条目。dev 标记说明构建依赖图；其他条目属于 production 依赖图。Vite/Svelte 会编译、内联和消除代码，锁文件不能证明最终保留范围。
4. 未分析最终 app/helper 的链接映射与前端 bundle。不覆盖系统 SDK、编译器、系统动态库、Claude 应用及其账号数据。
5. 发行负责人仍须确认目标、features、实际工件、许可选择及再分发义务，并补齐适用包的缺失正文。

## 收集方法与限制

Cargo license 标识优先取精确版本的缓存 Cargo.toml，再取同版本本地 sparse registry index。index checksum 必须匹配 Cargo.lock。
Cargo 正文优先取本地 .crate 原始包，完整归档 SHA256 必须等于 Cargo.lock checksum。否则取源码缓存，有 .cargo-checksum.json 时校验；无校验材料时单独标注。npm 标识优先取锁文件；本地 package.json 必须同名同版本。
npm 正文来自已安装目录，未对 npm tarball integrity 作独立复算。清单记录原始 integrity、文本文件路径和 SHA256。
递归收集名称为 LICENSE/LICENCE/NOTICE/COPYRIGHT 的文件，以及 Cargo license-file 指定文件；跳过 symlink、嵌套 node_modules、.git 与 target。
这是按文件名采集，不能保证捕获仅存在源码头注释中的声明或任意命名的许可证。双重许可表达式照录，未代替作者选择分支。

补充材料逐包记录精确 .crate VCS 提交或 npm 发布 gitHead，以及来源 URL、适用依据和文本摘要。同版本 CLI/rolldown 主包全文用于其精确版本平台绑定，并保留主包 tarball 对锁文件 SRI 校验依据。
objc2 系列和两个 Rich Harris 包没有上游项目专属完整版权通知。材料保留精确提交的原始许可声明，并附官方标准许可条款，保留版权占位符。**条款已归档不代表这 13 项通知缺口关闭。**
这些包在 inventory 的 upstream_notice_review_required 中单列。不得用规范模板代替真实版权署名，不得把此统计作为法律审定。详见 [补充证据说明](supplements/README.md)。

## 本次覆盖

| 生态 | 第三方包条目 | 已归档许可材料 | 材料缺项 | 上游专属通知待核 | 标识 UNKNOWN |
|---|---:|---:|---:|---:|---:|
| cargo | 508 | 360 | 148 | 11 | 145 |
| npm | 84 | 50 | 34 | 2 | 0 |

正文缺项逐项见 [MISSING.md](notices/MISSING.md)。所有条目的文件归属、摘要和证据来源见 [inventory.json](notices/inventory.json)。
完整原文：[Cargo](notices/cargo-license-texts.txt)、[npm](notices/npm-license-texts.txt)。分隔行提供包名、版本和原始文件名，正文保持 UTF-8 原文。

## 锁文件不变性

生成前后 SHA256 一致：

| 文件 | SHA256 |
|---|---|
| `Cargo.lock` | `b535d6041b83fa8f5d203ba91829c2faf44cfb34530f2ae7bce2405de7c50f5c` |
| `apps/desktop/package-lock.json` | `a7062c06e261a8ce4834fd23cb4506326802ad82ecea61f4b1a5ab6435b569e4` |

## 复现

在项目根目录执行 `python3 packaging/notices/generate.py`。需要 Python 3.11+ 和相同锁文件、本地依赖缓存。
可用 `--cargo-registry /path/to/.cargo/registry` 明确指定缓存。脚本不联网。缓存覆盖变化会改变正文覆盖计数。

## cargo 锁定清单

| 包 | 版本 | 来源 | license 标识 | 图边界 | 正文文件数 |
|---|---|---|---|---|---:|
| adler2 | 2.0.1 | registry+https://github.com/rust-lang/crates.io-index | 0BSD OR MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| aead | 0.5.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| aho-corasick | 1.1.5 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| alloc-no-stdlib | 3.0.0 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| alloc-stdlib | 0.3.0 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| android_system_properties | 0.1.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| anyhow | 1.0.104 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| arbitrary | 1.5.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| argon2 | 0.5.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| atk | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| atk-sys | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| atomic-waker | 1.1.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| autocfg | 1.5.1 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| aws-lc-rs | 1.18.2 | registry+https://github.com/rust-lang/crates.io-index | ISC AND (Apache-2.0 OR ISC) | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| aws-lc-sys | 0.46.0 | registry+https://github.com/rust-lang/crates.io-index | ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| base64 | 0.21.7 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| base64 | 0.22.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| base64 | 0.23.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| base64ct | 1.8.3 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| bit-set | 0.8.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| bit-vec | 0.8.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| bitflags | 1.3.2 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| bitflags | 2.13.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| blake2 | 0.10.6 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| block-buffer | 0.10.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| block-buffer | 0.12.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| block2 | 0.6.2 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| brotli | 9.0.0 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause AND MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| brotli-decompressor | 6.0.1 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause/MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| bs58 | 0.5.1 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| bumpalo | 3.20.3 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| bytemuck | 1.25.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| byteorder | 1.5.0 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| byteorder-lite | 0.1.0 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| bytes | 1.12.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| cairo-rs | 0.18.5 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| cairo-sys-rs | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| camino | 1.2.6 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cargo-platform | 0.1.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cargo_metadata | 0.19.2 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| cargo_toml | 1.0.1 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| cc | 1.9.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cesu8 | 1.1.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| cfb | 0.14.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| cfg-expr | 0.15.8 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| cfg-if | 1.0.5 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cfg_aliases | 0.2.2 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| chacha20 | 0.9.1 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| chacha20 | 0.10.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| chacha20poly1305 | 0.10.1 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| chrono | 0.4.45 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| cipher | 0.4.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cmake | 0.1.58 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| combine | 4.6.9 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| const-oid | 0.10.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cookie | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| core-foundation | 0.9.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| core-foundation | 0.10.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| core-foundation-sys | 0.8.7 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| core-graphics | 0.25.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| core-graphics-types | 0.2.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cpufeatures | 0.2.17 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cpufeatures | 0.3.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| crc32fast | 1.5.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| crossbeam-channel | 0.5.17 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| crossbeam-utils | 0.8.23 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| crypto-common | 0.1.7 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| crypto-common | 0.2.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| cssparser | 0.37.0 | registry+https://github.com/rust-lang/crates.io-index | MPL-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| cssparser-macros | 0.7.1 | registry+https://github.com/rust-lang/crates.io-index | MPL-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| ctor | 1.0.13 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| curve25519-dalek | 5.0.0 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| curve25519-dalek-derive | 0.1.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| darling | 0.24.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| darling_core | 0.24.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| darling_macro | 0.24.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| dbus | 0.9.12 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| defmt | 1.2.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| defmt-macros | 1.2.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| defmt-parser | 1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| deranged | 0.5.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| derive_arbitrary | 1.5.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| derive_more | 2.1.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| derive_more-impl | 2.1.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| digest | 0.10.7 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| digest | 0.11.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| dirs | 7.0.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| dirs-sys | 0.5.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| dispatch2 | 0.3.1 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| displaydoc | 0.2.7 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| dlopen2 | 0.8.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| dlopen2_derive | 0.4.3 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| dom_query | 0.28.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| dpi | 0.1.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 AND MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| dtoa | 1.0.11 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| dtoa-short | 0.3.5 | registry+https://github.com/rust-lang/crates.io-index | MPL-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| dunce | 1.0.5 | registry+https://github.com/rust-lang/crates.io-index | CC0-1.0 OR MIT-0 OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| dyn-clone | 1.0.20 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| ed25519 | 3.0.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| ed25519-dalek | 3.0.0 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| embed-resource | 3.0.12 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| embed_plist | 1.2.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| equivalent | 1.0.3 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| erased-serde | 0.4.10 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| errno | 0.3.14 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| fastrand | 2.5.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| fdeflate | 0.3.7 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| fiat-crypto | 0.3.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| field-offset | 0.3.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| filetime | 0.2.29 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| find-msvc-tools | 0.1.15 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| flate2 | 1.1.10 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| fnv | 1.0.7 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 / MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| foldhash | 0.2.0 | registry+https://github.com/rust-lang/crates.io-index | Zlib | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| foreign-types | 0.5.0 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| foreign-types-macros | 0.2.4 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| foreign-types-shared | 0.3.1 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| form_urlencoded | 1.2.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| fs_extra | 1.3.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| futures-channel | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| futures-core | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| futures-executor | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| futures-io | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| futures-macro | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| futures-sink | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| futures-task | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| futures-util | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| gdk | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gdk-pixbuf | 0.18.5 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gdk-pixbuf-sys | 0.18.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gdk-sys | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gdkwayland-sys | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gdkx11 | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gdkx11-sys | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| generic-array | 0.14.7 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| getrandom | 0.2.17 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| getrandom | 0.3.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| getrandom | 0.4.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| gio | 0.18.4 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gio-sys | 0.18.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| glib | 0.18.5 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| glib-macros | 0.18.5 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| glib-sys | 0.18.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| glob | 0.3.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| gobject-sys | 0.18.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gtk | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gtk-sys | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| gtk3-macros | 0.18.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| hashbrown | 0.12.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| hashbrown | 0.17.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| heck | 0.4.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| heck | 0.5.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| hex | 0.4.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| html5ever | 0.39.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| http | 1.5.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| http-body | 1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| http-body-util | 0.1.5 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| httparse | 1.10.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| hybrid-array | 0.4.15 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| hyper | 1.12.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| hyper-rustls | 0.27.10 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR ISC OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| hyper-util | 0.1.21 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| iana-time-zone | 0.1.65 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| iana-time-zone-haiku | 0.1.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| ico | 0.6.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| icu_collections | 2.3.0 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| icu_locale_core | 2.3.0 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| icu_normalizer | 2.3.0 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| icu_normalizer_data | 2.3.0 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| icu_properties | 2.3.0 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| icu_properties_data | 2.3.0 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| icu_provider | 2.3.1 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| ident_case | 1.0.1 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| idna | 1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| idna_adapter | 1.2.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| image | 0.25.10 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| indexmap | 1.9.3 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| indexmap | 2.14.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| infer | 0.22.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| inout | 0.1.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| ipnet | 2.12.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| itoa | 1.0.18 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| javascriptcore-rs | 1.1.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| javascriptcore-rs-sys | 1.1.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jiff | 0.2.38 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| jiff-core | 0.1.1 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| jiff-static | 0.2.38 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jiff-tzdb | 0.1.9 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jiff-tzdb-platform | 0.1.3 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jni | 0.21.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jni | 0.22.4 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jni-macros | 0.22.4 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jni-sys | 0.3.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jni-sys | 0.4.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jni-sys-macros | 0.4.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| jobserver | 0.1.35 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| js-sys | 0.3.106 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| json-patch | 4.2.0 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| jsonptr | 0.7.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| keyboard-types | 0.8.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| libappindicator | 0.9.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| libappindicator-sys | 0.9.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| libc | 0.2.191 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| libdbus-sys | 0.2.7 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| libloading | 0.7.4 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| libredox | 0.1.25 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| linux-raw-sys | 0.12.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| litemap | 0.8.3 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| lock_api | 0.4.14 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| log | 0.4.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| lru-slab | 0.1.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 OR Zlib | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| markup5ever | 0.39.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| memchr | 2.8.3 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| memoffset | 0.9.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| mime | 0.3.17 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| minisign-verify | 0.3.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| miniz_oxide | 0.8.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Zlib OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| miniz_oxide | 0.9.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Zlib OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| mio | 1.2.4 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| moxcms | 0.8.1 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| muda | 0.20.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| ndk | 0.9.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| ndk-context | 0.1.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| ndk-sys | 0.6.0+11769913 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| new_debug_unreachable | 1.0.6 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| num-conv | 0.2.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| num-traits | 0.2.19 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| num_enum | 0.7.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| num_enum_derive | 0.7.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2 | 0.6.5 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| objc2-app-kit | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| objc2-cloud-kit | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-core-data | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-core-foundation | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| objc2-core-graphics | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-core-image | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-core-location | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-core-text | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-encode | 4.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| objc2-exception-helper | 0.1.1 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| objc2-foundation | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| objc2-io-surface | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-osa-kit | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| objc2-quartz-core | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| objc2-ui-kit | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-user-notifications | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| objc2-web-kit | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| once_cell | 1.21.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| opaque-debug | 0.3.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| openssl-probe | 0.2.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| option-ext | 0.2.0 | registry+https://github.com/rust-lang/crates.io-index | MPL-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| osakit | 0.3.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| pango | 0.18.3 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| pango-sys | 0.18.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| parking_lot | 0.12.5 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| parking_lot_core | 0.9.12 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| password-hash | 0.5.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| percent-encoding | 2.3.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| phf | 0.13.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| phf_codegen | 0.13.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| phf_generator | 0.13.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| phf_macros | 0.13.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| phf_shared | 0.13.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| pin-project-lite | 0.2.17 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| pkg-config | 0.3.34 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| plist | 1.10.3 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| png | 0.18.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| poly1305 | 0.8.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| portable-atomic | 1.15.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| portable-atomic-util | 0.2.8 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| potential_utf | 0.1.6 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| powerfmt | 0.2.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| precomputed-hash | 0.1.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| proc-macro-crate | 1.3.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| proc-macro-crate | 2.0.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| proc-macro-crate | 3.5.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| proc-macro-error | 1.0.4 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| proc-macro-error-attr | 1.0.4 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| proc-macro2 | 1.0.107 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| pxfm | 0.1.30 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| quick-xml | 0.42.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| quinn | 0.11.12 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| quinn-proto | 0.11.19 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| quinn-udp | 0.5.16 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| quote | 1.0.47 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| r-efi | 5.3.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| r-efi | 6.0.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| rand | 0.10.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| rand_core | 0.6.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| rand_core | 0.10.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| rand_pcg | 0.10.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| raw-window-handle | 0.6.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 OR Zlib | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| redox_syscall | 0.5.18 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| redox_users | 0.5.3 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| ref-cast | 1.0.27 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| ref-cast-impl | 1.0.27 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| regex | 1.13.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| regex-automata | 0.4.18 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| regex-syntax | 0.8.11 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| reqwest | 0.13.5 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| ring | 0.17.14 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 AND ISC | overinclusive: targets/features/build/proc-macro/runtime unresolved | 6 |
| rustc-hash | 2.1.3 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| rustc_version | 0.4.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| rustix | 1.1.5 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| rustls | 0.23.46 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR ISC OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| rustls-native-certs | 0.8.5 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| rustls-pki-types | 1.15.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| rustls-platform-verifier | 0.7.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| rustls-platform-verifier-android | 0.2.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| rustls-webpki | 0.103.16 | registry+https://github.com/rust-lang/crates.io-index | ISC | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| rustversion | 1.0.22 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| same-file | 1.0.6 | registry+https://github.com/rust-lang/crates.io-index | Unlicense/MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| schannel | 0.1.30 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| schemars | 0.8.22 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| schemars | 0.9.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| schemars | 1.2.3 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| schemars_derive | 0.8.22 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| scopeguard | 1.2.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| security-framework | 3.7.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| security-framework-sys | 2.17.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| selectors | 0.38.0 | registry+https://github.com/rust-lang/crates.io-index | MPL-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| semver | 1.0.28 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde | 1.0.229 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde-untagged | 0.1.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_core | 1.0.229 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_derive | 1.0.229 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_derive_internals | 0.29.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_json | 1.0.154 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_repr | 0.1.21 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_spanned | 0.6.9 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| serde_spanned | 1.1.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_with | 3.24.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serde_with_macros | 3.24.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serialize-to-javascript | 0.1.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| serialize-to-javascript-impl | 0.1.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| servo_arc | 0.4.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| sha2 | 0.10.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| sha2 | 0.11.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| shlex | 2.0.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| signal-hook-registry | 1.4.8 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| signature | 3.0.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| simd-adler32 | 0.3.10 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| simd_cesu8 | 1.2.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| simdutf8 | 0.1.5 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| siphasher | 1.0.4 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| slab | 0.4.12 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| smallvec | 1.16.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| socket2 | 0.6.5 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| softbuffer | 0.4.8 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| soup3 | 0.5.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| soup3-sys | 0.5.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| stable_deref_trait | 1.2.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| static_vcruntime | 3.0.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 OR Zlib | overinclusive: targets/features/build/proc-macro/runtime unresolved | 4 |
| string_cache | 0.9.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| string_cache_codegen | 0.6.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| strsim | 0.11.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| subtle | 2.6.1 | registry+https://github.com/rust-lang/crates.io-index | BSD-3-Clause | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| swift-rs | 1.0.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| syn | 1.0.109 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| syn | 2.0.119 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| syn | 3.0.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| sync_wrapper | 1.0.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| synstructure | 0.14.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| system-configuration | 0.7.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| system-configuration-sys | 0.6.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| system-deps | 6.2.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| tao | 0.37.1 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tao-macros | 0.1.4 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| tar | 0.4.46 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| target-lexicon | 0.12.16 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| tauri | 2.12.5 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-build | 2.7.4 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-codegen | 2.7.4 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-macros | 2.7.4 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-plugin | 2.7.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-plugin-updater | 2.14.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| tauri-runtime | 2.12.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-runtime-wry | 2.12.3 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-utils | 2.10.2 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tauri-winres | 0.3.7 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tempfile | 3.27.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tendril | 0.5.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| termcolor | 1.4.1 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| thiserror | 1.0.69 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| thiserror | 2.0.21 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| thiserror-impl | 1.0.69 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| thiserror-impl | 2.0.21 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| time | 0.3.55 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| time-core | 0.1.9 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| time-macros | 0.2.32 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tinystr | 0.8.4 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tinyvec | 1.13.3 | registry+https://github.com/rust-lang/crates.io-index | Zlib OR Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| tokio | 1.54.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tokio-macros | 2.7.2 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tokio-rustls | 0.26.6 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tokio-util | 0.7.20 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| toml | 0.8.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| toml | 1.1.8+spec-1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| toml_datetime | 0.6.3 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| toml_datetime | 1.1.2+spec-1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| toml_edit | 0.19.15 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| toml_edit | 0.20.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| toml_edit | 0.25.17+spec-1.1.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| toml_parser | 1.1.5+spec-1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| toml_writer | 1.1.3+spec-1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tower | 0.5.3 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tower-http | 0.6.11 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tower-layer | 0.3.3 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tower-service | 0.3.3 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tracing | 0.1.44 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| tracing-core | 0.1.36 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| tray-icon | 0.25.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| try-lock | 0.2.5 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| ts-rs | 12.0.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| ts-rs-macros | 12.0.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| typeid | 1.0.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| typenum | 1.20.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| unicode-ident | 1.0.27 | registry+https://github.com/rust-lang/crates.io-index | (MIT OR Apache-2.0) AND Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| unicode-segmentation | 1.13.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| universal-hash | 0.5.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| untrusted | 0.9.0 | registry+https://github.com/rust-lang/crates.io-index | ISC | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| url | 2.5.8 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| urlpattern | 0.6.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| utf8_iter | 1.0.4 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| uuid | 1.28.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| version-compare | 0.2.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| version_check | 0.9.5 | registry+https://github.com/rust-lang/crates.io-index | MIT/Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| vswhom | 0.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| vswhom-sys | 0.1.3 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| walkdir | 2.5.0 | registry+https://github.com/rust-lang/crates.io-index | Unlicense/MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| want | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| wasi | 0.11.1+wasi-snapshot-preview1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wasip2 | 1.0.4+wasi-0.2.12 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wasm-bindgen | 0.2.129 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wasm-bindgen-futures | 0.4.79 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wasm-bindgen-macro | 0.2.129 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wasm-bindgen-macro-support | 0.2.129 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wasm-bindgen-shared | 0.2.129 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wasm-streams | 0.5.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| web-sys | 0.3.106 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| web-time | 1.1.0 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| web_atoms | 0.2.6 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| webkit2gtk | 2.0.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| webkit2gtk-sys | 2.0.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| webpki-root-certs | 1.0.10 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| webview2-com | 0.39.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| webview2-com-macros | 0.8.1 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| webview2-com-sys | 0.39.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| winapi | 0.3.9 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| winapi-i686-pc-windows-gnu | 0.4.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| winapi-util | 0.1.11 | registry+https://github.com/rust-lang/crates.io-index | Unlicense OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| window-vibrancy | 0.8.1 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| windows | 0.62.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-collections | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-core | 0.62.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-future | 0.3.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-implement | 0.60.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-interface | 0.59.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-link | 0.2.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-numerics | 0.3.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-registry | 0.6.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-result | 0.4.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-strings | 0.5.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-sys | 0.45.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-sys | 0.52.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-sys | 0.61.2 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-targets | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-targets | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows-threading | 0.2.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows-version | 0.1.7 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| windows_aarch64_gnullvm | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_aarch64_gnullvm | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_aarch64_msvc | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_aarch64_msvc | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_i686_gnu | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_i686_gnu | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_i686_gnullvm | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_i686_msvc | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_i686_msvc | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_x86_64_gnu | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_x86_64_gnu | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_x86_64_gnullvm | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_x86_64_gnullvm | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_x86_64_msvc | 0.42.2 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| windows_x86_64_msvc | 0.52.6 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| winnow | 0.5.40 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| winnow | 1.0.4 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| winreg | 0.56.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| wit-bindgen | 0.57.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| writeable | 0.6.4 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| wry | 0.57.0 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| x11 | 2.21.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| x11-dl | 2.21.0 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| xattr | 1.6.1 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| yoke | 0.8.3 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| yoke-derive | 0.8.4 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zerofrom | 0.1.8 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zerofrom-derive | 0.1.8 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zeroize | 1.9.1 | registry+https://github.com/rust-lang/crates.io-index | Apache-2.0 OR MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 2 |
| zerotrie | 0.2.5 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zerovec | 0.11.8 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zerovec-derive | 0.11.6 | registry+https://github.com/rust-lang/crates.io-index | Unicode-3.0 | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zip | 4.6.1 | registry+https://github.com/rust-lang/crates.io-index | UNKNOWN | overinclusive: targets/features/build/proc-macro/runtime unresolved | 0 |
| zlib-rs | 0.6.8 | registry+https://github.com/rust-lang/crates.io-index | Zlib | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zmij | 1.0.23 | registry+https://github.com/rust-lang/crates.io-index | MIT | overinclusive: targets/features/build/proc-macro/runtime unresolved | 1 |
| zune-core | 0.5.3 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 OR Zlib | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |
| zune-jpeg | 0.5.15 | registry+https://github.com/rust-lang/crates.io-index | MIT OR Apache-2.0 OR Zlib | overinclusive: targets/features/build/proc-macro/runtime unresolved | 3 |

## npm 锁定清单

| 包 | 版本 | 来源 | license 标识 | 图边界 | 正文文件数 |
|---|---|---|---|---|---:|
| @jridgewell/gen-mapping | 0.3.13 | https://registry.npmjs.org/@jridgewell/gen-mapping/-/gen-mapping-0.3.13.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| @jridgewell/remapping | 2.3.5 | https://registry.npmjs.org/@jridgewell/remapping/-/remapping-2.3.5.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| @jridgewell/resolve-uri | 3.1.2 | https://registry.npmjs.org/@jridgewell/resolve-uri/-/resolve-uri-3.1.2.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| @jridgewell/sourcemap-codec | 1.6.0 | https://registry.npmjs.org/@jridgewell/sourcemap-codec/-/sourcemap-codec-1.6.0.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| @jridgewell/trace-mapping | 0.3.31 | https://registry.npmjs.org/@jridgewell/trace-mapping/-/trace-mapping-0.3.31.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| @oxc-project/types | 0.155.0 | https://registry.npmjs.org/@oxc-project/types/-/types-0.155.0.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| @rolldown/binding-android-arm-eabi | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-android-arm-eabi/-/binding-android-arm-eabi-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-android-arm64 | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-android-arm64/-/binding-android-arm64-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-darwin-arm64 | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-darwin-arm64/-/binding-darwin-arm64-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 2 |
| @rolldown/binding-darwin-x64 | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-darwin-x64/-/binding-darwin-x64-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-freebsd-x64 | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-freebsd-x64/-/binding-freebsd-x64-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-linux-arm-gnueabihf | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-linux-arm-gnueabihf/-/binding-linux-arm-gnueabihf-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-linux-arm64-gnu | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-linux-arm64-gnu/-/binding-linux-arm64-gnu-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-linux-arm64-musl | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-linux-arm64-musl/-/binding-linux-arm64-musl-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-linux-ppc64-gnu | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-linux-ppc64-gnu/-/binding-linux-ppc64-gnu-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-linux-s390x-gnu | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-linux-s390x-gnu/-/binding-linux-s390x-gnu-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-linux-x64-gnu | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-linux-x64-gnu/-/binding-linux-x64-gnu-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-linux-x64-musl | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-linux-x64-musl/-/binding-linux-x64-musl-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-openharmony-arm64 | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-openharmony-arm64/-/binding-openharmony-arm64-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-win32-arm64-msvc | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-win32-arm64-msvc/-/binding-win32-arm64-msvc-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/binding-win32-x64-msvc | 1.2.15 | https://registry.npmjs.org/@rolldown/binding-win32-x64-msvc/-/binding-win32-x64-msvc-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 0 |
| @rolldown/pluginutils | 1.0.1 | https://registry.npmjs.org/@rolldown/pluginutils/-/pluginutils-1.0.1.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| @sveltejs/acorn-typescript | 1.0.13 | https://registry.npmjs.org/@sveltejs/acorn-typescript/-/acorn-typescript-1.0.13.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| @sveltejs/load-config | 0.2.3 | https://registry.npmjs.org/@sveltejs/load-config/-/load-config-0.2.3.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| @sveltejs/vite-plugin-svelte | 7.3.1 | https://registry.npmjs.org/@sveltejs/vite-plugin-svelte/-/vite-plugin-svelte-7.3.1.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| @tauri-apps/api | 2.12.4 | https://registry.npmjs.org/@tauri-apps/api/-/api-2.12.4.tgz | Apache-2.0 OR MIT | production dependency graph; bundled subset unresolved | 3 |
| @tauri-apps/cli | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli/-/cli-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 2 |
| @tauri-apps/cli-darwin-arm64 | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-darwin-arm64/-/cli-darwin-arm64-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 2 |
| @tauri-apps/cli-darwin-x64 | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-darwin-x64/-/cli-darwin-x64-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-linux-arm-gnueabihf | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-linux-arm-gnueabihf/-/cli-linux-arm-gnueabihf-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-linux-arm64-gnu | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-linux-arm64-gnu/-/cli-linux-arm64-gnu-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-linux-arm64-musl | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-linux-arm64-musl/-/cli-linux-arm64-musl-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-linux-riscv64-gnu | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-linux-riscv64-gnu/-/cli-linux-riscv64-gnu-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-linux-x64-gnu | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-linux-x64-gnu/-/cli-linux-x64-gnu-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-linux-x64-musl | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-linux-x64-musl/-/cli-linux-x64-musl-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-win32-arm64-msvc | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-win32-arm64-msvc/-/cli-win32-arm64-msvc-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-win32-ia32-msvc | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-win32-ia32-msvc/-/cli-win32-ia32-msvc-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @tauri-apps/cli-win32-x64-msvc | 2.12.4 | https://registry.npmjs.org/@tauri-apps/cli-win32-x64-msvc/-/cli-win32-x64-msvc-2.12.4.tgz | Apache-2.0 OR MIT | dev/build graph (npm dev flag) | 0 |
| @types/estree | 1.0.9 | https://registry.npmjs.org/@types/estree/-/estree-1.0.9.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| acorn | 8.19.0 | https://registry.npmjs.org/acorn/-/acorn-8.19.0.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| aria-query | 5.3.1 | https://registry.npmjs.org/aria-query/-/aria-query-5.3.1.tgz | Apache-2.0 | production dependency graph; bundled subset unresolved | 1 |
| axobject-query | 4.1.0 | https://registry.npmjs.org/axobject-query/-/axobject-query-4.1.0.tgz | Apache-2.0 | production dependency graph; bundled subset unresolved | 1 |
| chokidar | 4.0.3 | https://registry.npmjs.org/chokidar/-/chokidar-4.0.3.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| clsx | 2.1.1 | https://registry.npmjs.org/clsx/-/clsx-2.1.1.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| deepmerge | 4.3.1 | https://registry.npmjs.org/deepmerge/-/deepmerge-4.3.1.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| detect-libc | 2.1.2 | https://registry.npmjs.org/detect-libc/-/detect-libc-2.1.2.tgz | Apache-2.0 | dev/build graph (npm dev flag) | 1 |
| devalue | 5.9.4 | https://registry.npmjs.org/devalue/-/devalue-5.9.4.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| esm-env | 1.2.2 | https://registry.npmjs.org/esm-env/-/esm-env-1.2.2.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| esrap | 2.4.0 | https://registry.npmjs.org/esrap/-/esrap-2.4.0.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| fdir | 6.5.0 | https://registry.npmjs.org/fdir/-/fdir-6.5.0.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| fsevents | 2.3.3 | https://registry.npmjs.org/fsevents/-/fsevents-2.3.3.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| is-reference | 3.0.3 | https://registry.npmjs.org/is-reference/-/is-reference-3.0.3.tgz | MIT | production dependency graph; bundled subset unresolved | 2 |
| lightningcss | 1.33.0 | https://registry.npmjs.org/lightningcss/-/lightningcss-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 1 |
| lightningcss-android-arm64 | 1.33.0 | https://registry.npmjs.org/lightningcss-android-arm64/-/lightningcss-android-arm64-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-darwin-arm64 | 1.33.0 | https://registry.npmjs.org/lightningcss-darwin-arm64/-/lightningcss-darwin-arm64-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 1 |
| lightningcss-darwin-x64 | 1.33.0 | https://registry.npmjs.org/lightningcss-darwin-x64/-/lightningcss-darwin-x64-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-freebsd-x64 | 1.33.0 | https://registry.npmjs.org/lightningcss-freebsd-x64/-/lightningcss-freebsd-x64-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-linux-arm-gnueabihf | 1.33.0 | https://registry.npmjs.org/lightningcss-linux-arm-gnueabihf/-/lightningcss-linux-arm-gnueabihf-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-linux-arm64-gnu | 1.33.0 | https://registry.npmjs.org/lightningcss-linux-arm64-gnu/-/lightningcss-linux-arm64-gnu-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-linux-arm64-musl | 1.33.0 | https://registry.npmjs.org/lightningcss-linux-arm64-musl/-/lightningcss-linux-arm64-musl-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-linux-x64-gnu | 1.33.0 | https://registry.npmjs.org/lightningcss-linux-x64-gnu/-/lightningcss-linux-x64-gnu-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-linux-x64-musl | 1.33.0 | https://registry.npmjs.org/lightningcss-linux-x64-musl/-/lightningcss-linux-x64-musl-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-win32-arm64-msvc | 1.33.0 | https://registry.npmjs.org/lightningcss-win32-arm64-msvc/-/lightningcss-win32-arm64-msvc-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| lightningcss-win32-x64-msvc | 1.33.0 | https://registry.npmjs.org/lightningcss-win32-x64-msvc/-/lightningcss-win32-x64-msvc-1.33.0.tgz | MPL-2.0 | dev/build graph (npm dev flag) | 0 |
| locate-character | 3.0.0 | https://registry.npmjs.org/locate-character/-/locate-character-3.0.0.tgz | MIT | production dependency graph; bundled subset unresolved | 2 |
| magic-string | 1.4.3 | https://registry.npmjs.org/magic-string/-/magic-string-1.4.3.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| mri | 1.2.0 | https://registry.npmjs.org/mri/-/mri-1.2.0.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| nanoid | 3.3.20 | https://registry.npmjs.org/nanoid/-/nanoid-3.3.20.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| obug | 2.2.1 | https://registry.npmjs.org/obug/-/obug-2.2.1.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| picocolors | 1.1.1 | https://registry.npmjs.org/picocolors/-/picocolors-1.1.1.tgz | ISC | dev/build graph (npm dev flag) | 1 |
| picomatch | 4.0.7 | https://registry.npmjs.org/picomatch/-/picomatch-4.0.7.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| postcss | 8.5.31 | https://registry.npmjs.org/postcss/-/postcss-8.5.31.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| readdirp | 4.1.2 | https://registry.npmjs.org/readdirp/-/readdirp-4.1.2.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| rolldown | 1.2.15 | https://registry.npmjs.org/rolldown/-/rolldown-1.2.15.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| sade | 1.8.1 | https://registry.npmjs.org/sade/-/sade-1.8.1.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| source-map-js | 1.2.2 | https://registry.npmjs.org/source-map-js/-/source-map-js-1.2.2.tgz | BSD-3-Clause | dev/build graph (npm dev flag) | 1 |
| svelte | 5.57.2 | https://registry.npmjs.org/svelte/-/svelte-5.57.2.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| svelte-check | 4.7.7 | https://registry.npmjs.org/svelte-check/-/svelte-check-4.7.7.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| magic-string | 0.30.21 | https://registry.npmjs.org/magic-string/-/magic-string-0.30.21.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |
| tinyglobby | 0.2.17 | https://registry.npmjs.org/tinyglobby/-/tinyglobby-0.2.17.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| typescript | 6.0.3 | https://registry.npmjs.org/typescript/-/typescript-6.0.3.tgz | Apache-2.0 | dev/build graph (npm dev flag) | 1 |
| vite | 8.3.5 | https://registry.npmjs.org/vite/-/vite-8.3.5.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| vitefu | 1.1.3 | https://registry.npmjs.org/vitefu/-/vitefu-1.1.3.tgz | MIT | dev/build graph (npm dev flag) | 1 |
| zimmerframe | 1.1.5 | https://registry.npmjs.org/zimmerframe/-/zimmerframe-1.1.5.tgz | MIT | production dependency graph; bundled subset unresolved | 1 |

