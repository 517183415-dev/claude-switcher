# macOS arm64 发行依赖许可子集

本报告使用离线、锁定的 Cargo 查询。没有运行编译。全平台原始报告与许可正文保持独立。
它确认发行配置的依赖图与现有正文覆盖，不确认最终二进制实际包含的代码，也不代表许可放行。

## 查询边界

1. 目标固定为 `aarch64-apple-darwin`。根包包括 desktop、helper、core（CLI/guardian）。
2. 两版均使用 `--no-default-features`，分别启用 `switcher-desktop/personal` 或 `switcher-desktop/public`，并启用 Tauri release 使用的 `custom-protocol`。
3. `metadata.packages` 是包元数据集合；`resolve.nodes` 是带 feature 的解析节点集合。两者数量相同不代表所有包进入发行依赖图。
4. 本报告用 `cargo tree --edges normal,build` 的指定根包闭包作为有效发行构建图，排除 dev 边。它保留 build/proc-macro，尚未按最终链接结果删除构建工具。
5. npm 使用锁文件的 dependency、optional 和 peer 边，再核对当前 node_modules。两个发行共用前端依赖；edition 编译消除范围没有在本次核验中测量。

## Cargo 计数

| 发行 | metadata.packages | resolve.nodes | 有效图含第一方 | 第三方 | 归档材料缺项 | 上游专属通知待核 | 标识 UNKNOWN |
|---|---:|---:|---:|---:|---:|---:|---:|
| personal | 348 | 348 | 320 | 316 | 0 | 11 | 0 |
| public | 335 | 335 | 307 | 303 | 0 | 11 | 0 |

另行查询 `--edges normal`，区分只经 build 边可达的包。normal 图仍可含 proc-macro，不能称作实际运行时打包清单。
Cargo metadata 的 features 是解析节点观察值；它不会完整表达 resolver v2 的 host/target 编译单元拆分。

| 发行 | normal 边第三方 | 仅 build 图新增第三方 |
|---|---:|---:|
| personal | 298 | 18 |
| public | 285 | 18 |

### 有效图剩余 Cargo 归档材料缺项

| 包与版本 | 适用发行 | license 标识 | 缺项原因 |
|---|---|---|---|
| 无 | personal, public | — | 全部已有归档条款或声明；上游专属通知缺口另列 |

## npm 可达与当前安装状态

| 图 | 锁定可达包 | 当前已安装 | 已安装材料缺项 | 已安装上游通知待核 | 未安装可达包 |
|---|---:|---:|---:|---:|---:|
| production（含 optional/peer） | 20 | 20 | 0 | 2 | 0 |
| build（含 dev/optional/peer） | 84 | 50 | 0 | 2 | 34 |

当前已安装、且在 build 图中可达的 npm 正文缺项：

| 包 | 版本 | 图 | license |
|---|---|---|---|
| 无 | — | production + build | 全部已有归档许可材料 |

## 20 项原缺口的补充证据与剩余边界

5 个 Cargo 包已取得精确 VCS 提交的项目 LICENSE，或取得锁定源码明确引用的 Mozilla MPL-2.0 全文。两个 npm 平台绑定采用同版本主包完整许可证；主包 tarball 已对锁文件 SRI 校验。
另有 11 个 objc2 系列 crate，以及 is-reference、locate-character，只能取得精确提交的许可声明和官方标准条款。上游没有在这些提交提供项目专属版权通知。
**这 13 项上游通知缺口仍未关闭。** 标准许可保留版权占位符。没有填写作者姓名、没有制造 LICENSE，也没有把标准条款当作法律放行。
objc2 原始 LICENSE.md 另保留 Apple SDK 衍生条款适用性说明。该说明没有被删去或推断为已解决。

| 生态 | 包与版本 | 原始许可声明/版本依据 | 剩余项 |
|---|---|---|---|
| cargo | block2@0.6.2 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/b4167b582b2f75f9a1be75495c41b765344fd03c/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | dispatch2@0.3.1 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/8852b424193ca41602281b3d7540d7c8ed51e49a/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2@0.6.5 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/d7d2fa23ceaa5e6096c923b081040e5d81b3b9df/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-app-kit@0.3.2 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-core-foundation@0.3.2 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-encode@4.1.0 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/8d214f5477365ffcbcbb7de058c86ed9a518efb7/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-exception-helper@0.1.1 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/8d214f5477365ffcbcbb7de058c86ed9a518efb7/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-foundation@0.3.2 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-osa-kit@0.3.2 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-quartz-core@0.3.2 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| cargo | objc2-web-kit@0.3.2 | [exact commit](https://raw.githubusercontent.com/madsmtm/objc2/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| npm | is-reference@3.0.3 | [exact commit](https://raw.githubusercontent.com/Rich-Harris/is-reference/8bb053129bfabe2f6a7d7ed050159d67ebe82829/README.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |
| npm | locate-character@3.0.0 | [exact commit](https://raw.githubusercontent.com/Rich-Harris/locate-character/4f08a59ec248121f7002abd02ee7b94e8eda06bc/README.md) | 上游项目专属版权通知缺失；标准条款保留占位符 |

逐包来源 URL、精确版本/提交绑定、正文摘要与适用依据见 [supplements/sources.json](supplements/sources.json)。
补充过程与剩余边界见 [supplements/README.md](supplements/README.md)。现有 20 项均有条款/声明材料，但只有 7 项取得完整项目许可证或无占位符的明确引用许可；不要合并成‘20 项通知均已完整’。


未安装的 optional 平台包保留在锁图中，但不计入当前 node_modules 子集。完整名单与不可解析边见 JSON。
npm 前端生产图不等于产物代码清单；peer TypeScript 等工具可能在生产图可达，不能据此认定其运行时代码被打入。

## 证据与复现

完整子集条目、license、来源、正文 SHA256、解析 feature 与 metadata-only 差集见 [macos-arm64-subsets.json](macos-arm64-subsets.json)。
每版保留 `macos-arm64-*-metadata.json` 与 `macos-arm64-*-tree.txt`。metadata 文件只裁剪非图信息，保留 package IDs、resolve nodes、依赖边和 features。
运行 `python3 packaging/notices/macos_subset.py` 可复现。命令完整记录在 JSON；只调用 offline locked metadata/tree。
正文覆盖引用现有 inventory.json，不补写、不推断任何缺失 LICENSE。全平台材料仍见 [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)。

锁文件收集前后 SHA256 一致：

| 文件 | SHA256 |
|---|---|
| `Cargo.lock` | `b535d6041b83fa8f5d203ba91829c2faf44cfb34530f2ae7bce2405de7c50f5c` |
| `apps/desktop/package-lock.json` | `a7062c06e261a8ce4834fd23cb4506326802ad82ecea61f4b1a5ab6435b569e4` |
