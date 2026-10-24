# Keychain 依赖处理：v1.2 实现补充

交接 ID：CS-RUST-20261022-R4。本文件落实现行 v1.2，不增加包、任务、验收项或发行。
当前优先完成系统核心；广告后补。现有工作区修改、账号目录和冻结候选均保留。

## 1. 问题与目标

生产 `native_keychain::run()` 始终返回 `Unresolved`；真实账号交换没有可执行的依赖处理路径。
原位保存 A 的许可不能替代 A/B 交换。缺少第二账号不能解释这一实现缺口。

区分元数据观察、操作处理许可和真实登录验收。操作前确定处理方式；F-08/F-09 在真实操作后验收登录保持。
不再用“已证明登录保持”命名操作前的判定。属性摘要不代表秘密值一致，也不代表服务端会话有效。

```text
官方签名 + 已审查版本工件 + 限定属性观察
                    |
                    v
       核心依赖计划与一次性操作许可
                    |
      安装、退出、备份、目录、锁内复核
                    |
                    v
         现有目录事务与 journal 恢复
                    |
                    v
          独立真实 F-08 / F-09 验收
```

## 2. 当前版本的处理依据

只适配本机已调查的官方 Claude 3.1623.0 / Electron 44.9.0。精确工件路径、摘要、调用偏移与剩余限制见 [E07 公开工件证据](../acceptance/evidence/e07/official-claude-storage-contract.json)。
验证官方 Apple 签名、固定 Bundle/Team、版本及规则依赖的 ASAR、Electron Framework、原生绑定材料。
未知版本或材料不符时拒绝新交换；不能仅比较主程序和 Info.plist。

| 依赖 | 已见读取路径 | 本次处理 |
| --- | --- | --- |
| 共享加密材料 | Electron OSCrypt；service `Claude Safe Storage`，account `Claude` | 留在当前系统用户的 Keychain；必须观察到唯一匹配的可查询属性 |
| Claude 登录与加密账号资料 | `defaultSession.cookies` 的 `sessionKey`；safeStorage 密文配置 | 现有完整账号目录作为不透明资料保存、交换 |
| 硬件设备密钥 | account UUID 与目录内 generation 选择 tag；原生硬件签名绑定 | Keychain 密钥保持原位；UUID、generation 随完整目录保存 |
| WebAuthn 与连接器材料 | 固定 WebAuthn access group；MSAL 本地加密缓存 | 全部 Keychain 材料保持原位；不导出、删除、复制或重建；不宣称完成连接器或多 passkey 功能验收 |

静态调查未恢复完整原生 application-tag 构造，也未穷举 Microsoft broker 内部状态。
这限制具体功能的静态结论；不能把 entitlement 名称当作完备性证明。
首条可执行路径是同一系统用户、同一官方版本下的完整 Claude 账号目录交换。真实登录与设备接受仍须 F-08/F-09。
资料若必须跨用户、跨设备迁移，或需要同步改写全局凭据，本适配拒绝执行。

官方实现依据：

- [Electron 44.9.0 Keychain selector](https://raw.githubusercontent.com/electron/electron/v44.9.0/shell/browser/electron_browser_main_parts.cc)。
- [Chromium 152.0.7977.167 Keychain 加密材料](https://chromium.googlesource.com/chromium/src/+/152.0.7977.167/components/os_crypt/common/keychain_password_mac.mm)。

## 3. 最小实现边界

实现位于现有 `switcher-core`。不增加依赖，不新增自动登录验收框架。

1. 把版本契约、依赖类别和处理方式编译进核心。GUI/CLI 不能提交“已审查”布尔值、规则或有效许可。
2. 保留限定元数据查询。关闭 secret、引用和持久引用返回，禁止授权弹窗。增加精确共享材料 selector；零项、重复项、拒绝、超限和不完整均不签发新交换许可。
3. 使用私有、单次消费的交换许可。绑定 owner、发行、安装、intent、应用和契约、元数据、备份收据、账号状态、根/active/目标目录身份与摘要。
4. 生命周期锁前生成准备结果，锁内重新推导并比较。实际交换前再次核对退出状态、应用和相关绑定。沿用现有目录事务、身份检查和唯一逻辑提交点。
5. 恢复独立于新交换。已知 journal 的文件恢复验证 owner、安装、备份与目录身份，不能因新应用版本或 Keychain 查询不可用而要求先证明登录。恢复不会启动 Claude，也不授予下一次交换许可。

保持现有原位 A 初始化/关联的权限范围。不要给所有 `AccountStore` 调用增加不必要的新公共框架。
持久 journal 中的绑定摘要只用于恢复校验；从普通 JSON 反序列化不产生新的有效执行许可。

## 4. 必要回归

所有成功 fixture 经过生产依赖判定器；取消 `keychain_reviewed=true` 对交换能力的直接授权。

1. 当前版本与唯一共享材料能形成处理计划；未知版本、缺失/重复/拒绝属性及未知用途失败。
2. 同属性摘要不产生登录保持结论；许可绑定在锁内变化时拒绝执行。
3. 合成 A/B 真实目录交换经过生产协调器；秘密哨兵只在隔离目录内，零 Keychain secret 读写。
4. 交换前后中断、应用升级及属性查询失败仍可按有效 journal 恢复；身份冲突和坏 journal 保留资料并拒绝猜测。
5. 原位 A、备份、账户事务既有测试与两发行桌面投影继续通过；strict clippy、fmt、两版构建后再做必要原生验证。

## 5. 交付与剩余验收

先独立复审处理边界，再实现并复审源码。更新验签修复和本实现一起进入新候选；r3 证据不改写为新源码证据。
本轮合成与只读验证不写真实账号、不启动 Claude、不读取 Keychain 秘密、不安装系统组件。
完成条件是生产具备经版本契约判定的可执行交换路径，且必要故障回归通过。仅增加阻断文案不算完成。
正式身份、系统安装、真实 A、真实 A→B→A、长时运行保持独立状态，不能用本地合成通过替代。
