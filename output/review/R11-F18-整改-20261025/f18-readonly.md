# R11 / F18 只读消费审计

日期：2026-10-25。范围：准备状态兼容接口、生产消费及活动验收脚本。只读检查源码与 CodeGraph；没有运行测试、cargo/rustup、浏览器、产品、网络、账号或系统命令。没有修改业务源码。结论为静态事实，不能称运行复现或验收 PASS。

## 1. 结论与消费事实

**采纳 F18：准备状态存在确定的跨层矛盾。保留兼容入口，最小修正投影即可。** 未发现必须删除 API 的证据。仓库搜索不能证明外部工具没有消费者。

| 层 | 可验证事实 | 当前证据 |
|---|---|---|
| 核心 | readiness 固定 ready=true、空 blockers；Keychain 探针仅为 CLI 兼容，返回 Unavailable + VerifiedPreserved，无查询 | crates/switcher-core/src/migration.rs:47、:155 |
| 桌面 | 投影只接受 HandlingPlanAvailable；因此真实 VerifiedPreserved 被添加 keychain_dependencies_unresolved | apps/desktop/src-tauri/src/bridge.rs:580 |
| 备份 | 投影允许部分 blocker 下备份；实际 create_verified_backup 固定返回“旧版账号切换会自动存档，无需迁移备份或隔离恢复步骤” | bridge.rs:599；migration.rs:268 |
| IPC | migration_preparation / create_migration_backup 委托 bridge；public 与 personal 均注册，main capability 均允许 | commands.rs:43、:50；main.rs:129、:155；capabilities/main.json:14–15 |
| CLI | migration-preview 直接输出 readiness/accounts，并按 readiness.ready 返回 0/2；backup 调用同一个固定拒绝方法 | crates/switcher-core/src/bin/claude-switcher.rs:236、:246 |

调用关系：

```text
IPC migration_preparation → DesktopState → core.readiness → desktop projection
CLI migration-preview     → core.preview → core.readiness → JSON
IPC create_migration_backup ─┐
CLI backup                  ─┴→ core.create_verified_backup → 明确拒绝
```

普通 UI 生产代码未检出 migration_preparation、create_migration_backup 或 migration-preview 调用。搜索包含 apps/desktop/src、scripts、tests，使用 -uuu 覆盖忽略文件，并排除 generated。WorkspaceSnapshot 不含 preparation 字段（crates/switcher-domain/src/desktop.rs:58）。实际 App 自动检查由 bootstrap→runChecks 驱动（App.svelte:198、:207）。这些是本仓库消费事实，不覆盖用户脚本、旧安装版本或外部集成。

命令仍在 build.rs:13–14、:51–52 的权限生成清单。AccountPreparation DTO 在 domain/desktop.rs:183；导出在 domain/lib.rs:53；生成 TS 和自动权限均保留。删除这些内容会产生多文件兼容变更，当前没有必要。

## 2. 最小兼容结果建议

接受父会话四文件形态，无静态阻断：bridge.rs、commands.rs（若需调整边界提示）、e03_core_preparation.rs、activity-refresh-regression.py。若 commands.rs 不需要变更，可保持三文件。每组不超过五个源文件。

1. bridge 投影接受 VerifiedPreserved 和 HandlingPlanAvailable，不凭空追加 Keychain blocker。Unresolved 仍阻塞。任何 backend blocker 和 core.ready=false 必须原样约束 ready。
2. backup_allowed 固定 false。create_migration_backup 保持 completed=false，并提示无需迁移备份、使用正常账号选择与检查启动。不要提示“重新预览后即可备份”，也不要返回虚假 completed=true。
3. 保留命令名、DTO 字段、权限、CLI JSON 形状及原退出码。ready 只表示旧准备环节没有额外阻碍，不授予启动许可；启动仍走 launch_checked/switch_checked 的真实门禁。
4. 活动脚本按自动检查后 1 条、手动检查后 2 条调整。两条事件文本相同，原 :50 exact getByText().waitFor() 将多匹配；一并限定 first() 或等待 count。同步 :107 旧 empty 文案。

不需要改 MigrationCoordinator 的事务逻辑，不需要重新设计准备框架，不需要新增 DTO 字段或删除公共 API。

## 3. 活动脚本与历史资料

| 文件 | 分类 | 事实与处理 |
|---|---|---|
| apps/desktop/scripts/core-preparation-regression.mjs | npm test 活跃入口 | package.json:9 直接运行。脚本 :39–56 已断言开窗自动检查、fixture 零 IPC、首次 B 单动作切换。名字旧，但行为正确，不应删除或大改。 |
| apps/desktop/scripts/activity-refresh-regression.py | 独立可执行验收脚本 | :86 有 main；:28–30 每次检查追加事件；:42–45 却假设开页后为空，与自动检查冲突。不是 npm test 默认入口。脚本会启 Vite/浏览器，本轮未运行。 |
| apps/desktop/src-tauri/tests/e03_core_preparation.rs | 现存 Rust 投影测试 | :217 要求 VerifiedPreserved 阻塞，直接固化当前矛盾。应改为真实兼容输入通过，同时保留未知/未解决/后端阻塞的负向断言。 |
| output/review/**、历史报告与红绿源码快照 | 历史证据 | 不作为现行生产消费者，不改写、不删除、不通过关键词清理。 |

## 4. 明确不可删边界

| 数据/逻辑 | 现行恢复链 | 边界 |
|---|---|---|
| 旧 in-place journal | migration.rs:231–267 读取 v2/migration/in-place-journal.json，接受旧 InitializeA/AssociateA 并 finish_initialization | 准备 UI 退役不等于旧事务不存在。保留解析、校验、恢复和成功后删除顺序。 |
| account journal | accounts.rs:366 v2/account-journal.json；:633 recover_legacy_checked 先 legacy::recover 再 recover_checked | 保留 active-state、legacy 事务、账号目录与恢复门禁。 |
| repair journal | user_repairs.rs:404–411 v2/user-repairs/journal.json；:913 recover_with_lock | 保留 committed/rolled_back 收尾和恢复。PreparedUserRepair / RepairPreview 是真实修复计划，不属 F18 废弃准备状态。 |
| install journal | packaging/macos/local_install.py:35 source-local-transaction.json；:1866 recover(owner) | 保留 owner、stage、terminal、backup_aborted、卸载/安装恢复。uninstall_prepared 是事务阶段，不属准备 UI。 |

禁止按 preparation/prepared 字面批量删除。initialize-a / associate-a / recover CLI 仍有实际账号恢复作用（binary:246–253；migration.rs:207–230）。

## 5. 后续隔离验证形态（本轮 NOT_RUN）

1. e03 投影：真实 KeychainMetadataProbe::run() 输入；两种兼容 assessment；Unresolved；全部 backend blocker；core.ready=false；backup_allowed 始终 false；DTO 不泄漏查询元数据。
2. 备份边界：现有明确 unsupported 结果，不进入成功支路，不暗示稍后重试就支持，不产生新资料。
3. 保留 core-preparation-regression 的实际 App VM 测试，锁定开窗自动检查一次与 fixture 零 IPC。既有 npm test 可在隔离镜像执行。
4. 活动脚本先做静态/合成 IPC 断言。实际 Chromium/Vite 验收须另获当前允许范围，本轮明确 NOT_RUN。
5. 父会话统一检查 personal/public 编译、格式与 e03；不运行真实 HOME 或账号入口。变更后复核此处源快照 hash，不能拿旧 hash 代表新实现。

静态置信度：高。外部 CLI/旧安装消费者完整清单：未知。真实 GUI/原生 IPC/浏览器运行结果：未验证。

## 只读快照 SHA-256

以下为笔记落盘时读取的文件哈希，不是测试或冻结证明。

```
4c632b31a6d0b4c57d8379fba81f490d8b526447c8b71f72b3d11fcc1d90fae6  apps/desktop/src-tauri/src/bridge.rs
f0fb2d2290fb2d4718532f3c78347f4fd63b1a056b07a17aed12e6283207f562  apps/desktop/src-tauri/src/commands.rs
b38454fd24a996265ddc3cb04d675bd26a4b0b43b1d5a4632056fcd56951f763  apps/desktop/src-tauri/src/main.rs
8a2720b8245737d90c5be9df0f693ec0d6ee72db9cfc000d6ffd987df9cef82d  apps/desktop/src-tauri/tests/e03_core_preparation.rs
cfc55b035d9876441983fa215d1dc6de656b1b5960d358783e1e09ff5ae10841  apps/desktop/src-tauri/build.rs
02b2dc1e2b05aaed6aa74eb5a1b8476585b7f4d589774f4659ef420cf6e2f3e7  apps/desktop/src-tauri/capabilities/main.json
bb8f4a3285bf37260775f44a1df1a67e0fc736f55c0b34417d572f78d131c20a  crates/switcher-domain/src/desktop.rs
b7634f4c7e0404f961291d9336ff54533763ff4c03f3eeb1155d78a223fc989c  crates/switcher-domain/src/lib.rs
ffa668befc4e75f816b1c23ebb93bb22e46a416e0243889c92f1b45fb49efd9a  crates/switcher-core/src/migration.rs
f2ffa888354ef2e8ab61a9f3d778224dc3f90ba30188ffc63159f0409f4db907  crates/switcher-core/src/bin/claude-switcher.rs
8ed2be911b375c8111915f0030bff5c75e109f4c4fe1b805056209f5d77435fb  apps/desktop/scripts/core-preparation-regression.mjs
994f05f0a959681f9c8afe00fc4c7e3837d783f5352d7d068dc4d6b1ccbe6107  apps/desktop/scripts/activity-refresh-regression.py
682fc80f8e2cef117810ba4f27c73f9decd9b2beebad95605dfb7dc760a08a9a  apps/desktop/package.json
```
