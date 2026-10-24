# Rust 本地构建与系统组件安装


## 从源码构建

在 macOS 普通用户会话中准备 Xcode Command Line Tools、Rust、Node/npm 和 Python 3。Rust 版本由仓库 `rust-toolchain.toml` 指定。构建时不使用 sudo。

```bash
bash scripts/build-rust.sh personal
# 另一个发行：
bash scripts/build-rust.sh public
```

Apple Silicon 产物位于 `target/aarch64-apple-darwin/release/bundle/macos/`；Intel 路径为 `target/x86_64-apple-darwin/release/bundle/macos/`。构建脚本只生成本机原生架构。实际 Intel 支持须另行验收。

每个 App 内含已签名 helper、guardian 和 Rust CLI。工件启用 Hardened Runtime。未配置商业签名身份时使用 ad-hoc。本地组件安装需要管理员权限；普通源码核心操作不要求重新登记 GUI 指纹。helper 内部 XPC 仍保留精确身份验证。

## 安装、重装与换版

从仓库根目录运行。以下示例适用于 Apple Silicon 的 Personal 构建：

```bash
sudo python3 packaging/macos/local_install.py install --edition personal \
  --app "target/aarch64-apple-darwin/release/bundle/macos/Claude 切换器 Personal.app"
python3 packaging/macos/local_install.py verify --edition personal
```

`install` 默认替换已有组件。`replace` 保留为兼容命令。安装器先退出当前 owner 的 Claude 和切换器，再获取事务锁。它自动生成并校验内部计划摘要。普通 `verify` 无需 sudo，不退出应用，也不修改系统。

需要先审阅计划时，可选运行 `plan`：

```bash
python3 packaging/macos/local_install.py plan --edition personal \
  --app "target/aarch64-apple-darwin/release/bundle/macos/Claude 切换器 Personal.app"
```

计划包含固定目标、owner、工件 SHA256、代码指纹和摘要。将输出摘要作为可选 `--digest` 传入安装命令后，安装器会拒绝已经变化的计划。省略此参数时，安装器自动绑定当前计划。

首次安装不需要旧版脚本。计划优先采用已有 v2 配置和旧入口字面量；缺少地区配置时读取 Cloudflare 出口国家，再按 ipinfo → ipwho.is → ip.sb 顺序读取国家／时区，拒绝 CN、缺失数据或国家冲突，并映射地区格式。代理优先识别 Clash Verge（读取首个 select 组），其次识别 Shadowrocket，其他情况使用旧 auto 模式。地区发现 curl 仅继承 http(s)/all/no_proxy 的大小写环境变量；root 管理命令不继承这些变量。Clash Verge 支持缩进及无缩进的 proxy-groups 列表。探测只读，失败不写安装文件。计划固定五项配置，提交时不重复网络探测。

同一 owner 可直接使用 `install --edition public --app <Public App路径>` 换成 Public。安装器校验旧记录和旧 App 身份，备份旧 App，移除旧发行，再发布新发行。失败恢复会先移除新发行，再恢复旧发行及旧 CLI 入口。账号资料不随发行删除。管理员可在新 owner 的控制台会话直接安装并交接 owner，无需原 owner 先卸载。安装器按 uid 顺序锁定双方生命周期，只获取一次 root 锁；退出旧 owner 应用并撤下旧 guardian，更新唯一安装记录，再注册新 owner guardian。失败恢复旧记录、组件、双方原注册状态及新 owner CLI。旧 owner 的账号、配置和 CLI 内容不读取、不搬迁、不删除；普通运行和 XPC 的 owner 身份检查保持原有边界。卸载仍须对准当前安装 owner。

```text
校验与备份 → 退出旧服务 → 替换组件与旧入口 → 验证并提交
                   失败或中断 → recover 恢复原组件与入口
```

安装事务管理 App、helper、guardian、服务模板、安装记录及旧命令入口。`~/.local/bin/claude-switch.sh` 成为执行 Rust CLI 的 shim，保留 COUNTRY_TARGET、TZ_TARGET、LOCALE_TARGET、PROXY_MODE、PROXY_GROUP 字面量。旧脚本仅用于配置读取和备份，不执行旧核心逻辑。

旧 `local.tz-watchdog` root 时区守护保留。安装器根据已验证目标更新固定 root 脚本和 plist；新安装也会创建它。周期仍为 3600 秒并启用 RunAtLoad。旧 `local.claude-bodyguard` root 保镖也保留，按原模板每 10 秒运行，启用 RunAtLoad。它保留代理离线时终止其他登录用户 Claude 的旧范围；Rust 用户 guardian 同时保留。保镖模式只允许六个固定枚举，`auto` 仍只检查 TUN。两个 root 脚本固定安装在受保护系统目录，属 root:wheel、权限 755；plist 权限 644。安装失败会恢复旧脚本、plist 和注册状态，新装失败会撤掉新件。SourceLocal 的旧自动更新 Agent 停用。修复原值和回执文件原样保留，不作为重装条件。

安装完成验证组件身份、文件权限和服务进程，不等待 GUI／guardian 认证回执。`committed` 不证明 14 项安检全绿或账号切换成功，这些行为需要独立验收。

未完成事务可运行 `sudo python3 packaging/macos/local_install.py finalize` 继续组件验证，或运行 `sudo python3 packaging/macos/local_install.py recover` 恢复。恢复旧服务注册时，RunAtLoad 可能立即启动原任务。不要手工删除事务或备份目录。

卸载可直接运行 `sudo python3 packaging/macos/local_install.py uninstall --edition personal`。也可先运行 `plan --edition personal --uninstall-plan`，再传入可选摘要。卸载保留账号资料、配置资料和独立 root 时区守护及保镖。

旧 r12／r13 的管理员安装证据仅描述历史版本。本轮恢复目前完成隔离测试：91 项安装／打包测试（含 D05 换版与 G04 root 保镖定向恢复测试）、4 项 diagnostics Rust 测试。证据见 `output/tmp/legacy-restoration/install/`。当前源码版本尚需最终统一管理员安装及真实服务验收。

## 可信签名发布分支与恢复合同

以下保留 DeveloperId 的 PKG 和远程更新合同。它们不要求源码用户购买商业证书。

固定服务：`local.claudeswitcher.helper`（root LaunchDaemon/XPC）和
`local.claudeswitcher.guardian`（owner_uid 的 LaunchAgent）。

- helper 仅由 launchd 激活；不会自行注册服务。
- helper 二进制：`/Library/PrivilegedHelperTools/local.claudeswitcher.helper`。
- guardian 二进制：`/Library/Application Support/claude-switcher/switcher-guardian`。
- 安装记录：`/Library/Application Support/claude-switcher/install.json`，root 所有，禁止组/其他用户写入。
- guardian 心跳：`~/.claude-switcher/v2/guardian-health.json`，用户所有，0600。

安装记录 schema2 包含 owner_uid、edition、trust、helper_version、minimum_app_version。
SourceLocal 的 trust 保存本次 generation 和 GUI、guardian、helper 的 CDHash。DeveloperId 的 trust 保存固定 Team ID。owner_uid 来自本机管理员安装会话。
GUI、guardian 使用固定 Bundle ID。XPC 双端在激活前设置系统 peer requirement，每条消息再核对身份、版本、角色与会话。SourceLocal 拒绝 Installer 客户端角色。
测试的合成 VerifiedPeer 不导出构造接口。客户端不能传入布尔值代替签名验证。

`core::install::Installer` 是事务协调器。每步执行前落盘 pending，完成后落盘 completed。
授权取消不调用 backup/stage。失败或崩溃通过幂等 restore 恢复旧组件。
发行替换必须预览并明确使用 ReplaceEdition。卸载必须保留账号，恢复系统值前比较当前值与本工具写入值。

`SignedPkgBackend` 验证固定 Team 的 Installer 签名和 Gatekeeper 评估，再取得原生安装授权。
特权安装前把 PKG 复制到 root 专用临时路径，再次验证签名，避免用户目录替换竞态。
`build_pkg.py` 参数化两类 Developer ID 身份、Team、owner_uid、edition 和版本。
它只构建 components.pkg 与 recovery.pkg；`--operation uninstall` 生成卸载组件包。
`--plan` 不创建输出目录，不签名，不安装。`test_build_pkg.py` 仅验证计划、模板与脚本语法。

正常安装保留 root 回滚快照，直到签名 GUI 确认 helper 和 guardian 健康。
helper 写入持久提交标记后才确认提交。后续事务回收旧快照。
修复在 root `repair-journal.json` 记录原状态，启动时恢复中断修复。
root `system-original.json` 保留本工具修改前值；卸载仅恢复仍匹配本工具写入值的字段。

本次采用 SourceLocal 源码交付，无需 Developer ID 或公证。本地 ad-hoc 签名与 root 登记的精确代码指纹共同构成身份验证。
本地构建、隔离事务测试和管理员真机安装分别记录。实际状态见 [实施状态](../../docs/handoff/Rust-R4-实施状态.md)。DeveloperId 的 PKG、托管更新和公证分支保留，未执行不影响源码交付。

更新安装使用 `install::NativeUpdateCoordinator::new(root_v2, &lifecycle_lock, clock)`。
调用者必须持有同一个 `root/v2/lifecycle.lock`。先运行 `recover(&mut UpdateState)`，再将密封的
`updates::PreparedInstallation` 交给 `UpdateInstallCoordinator::install`。解包目录必须保留到调用返回。
协调器检查 Claude 已完全退出、账号事务空闲、时间可信和实际安装身份。
它将每个步骤写入 `root/v2/update-install-transaction.json`，并在根权限复制后重新校验 App。
App 暂存使用系统保护目录下随机 `0700` 私有目录。
App 备份位于 `/Library/Application Support/claude-switcher/app-{Personal|Public}-previous.app`。
执行器验证该目录与 `/Applications` 处于同一卷，并用排他原子重命名拒绝目的 symlink。
helper 的恢复包固定为 `root/v2/{personal|public}/update-recovery/helper.pkg`。
缺失该包时返回 `wait_update_recovery_pkg`，不会修改已安装组件。
只有实际恢复并重新验证旧 App/helper 健康后，状态才进入 `RolledBack`。

构建兼容升级 helper 时，显式传入 `--minimum-app-version <允许调用的新旧 GUI 最低版本>`。
默认值是当前包版本。该保守默认值可能拒绝旧 GUI，因此发布前必须校验元数据兼容范围和此值一致。
上述 NativeUpdateCoordinator 属于 DeveloperId 远程更新分支。SourceLocal 在进入协调器前拒绝远程安装，使用本地重建安装器升级。管理员取消、断电恢复及已注册 XPC 双进程往返仍需各自真机验收。
本地测试仅验证隔离事务、固定目的路径、命令引号和 shell 语法，不将其记为真实安装 PASS。

root 写入仅由签名 helper 的 root-only 固定枚举入口 `--root-operation` 执行。
GUI 使用已安装 helper；初次安装仅查找自身 bundle 的 `Contents/Resources/components/switcher-helper`。
执行前把该 helper 复制到 root 私有目录，并再次验证 Team 和固定 helper identifier。
root 执行器持有独立 `flock`，实际子进程继承该锁描述符。GUI 死亡不会放开正在运行的 root 操作。
PKG 必须通过该执行器安装。脚本拒绝缺少 root 安装门闩的直接安装。
`component-installer-active` 记录 boot UUID，覆盖 installer 客户端异常退出而 installd 仍工作的窗口。
同次开机的悬空门闩保持 WAIT；正常安装结束清除门闩。重新开机后才可确认旧进程已结束并恢复。
授权结束后和复制校验后，执行器重新验证元数据有效期、连续时钟、boot 与 Claude 退出状态。

修复结果协议：`ApplyRepair` 的等待预算为 150 秒，普通读取为 5 秒，
`RepairStatus { plan_id }` 为 20 秒。超时表示结果未知，取消 XPC 连接不会取消 root 任务。
root 在每个正向时区/IPv6 写入前重新检查实际外网路由，只有唯一 `utun` 数字接口可通过。
逐计划完成记录 `repair-results/<plan-id>.json` 先持久化，再清除 `repair-journal.json`；待恢复记录优先。
查询只返回同一计划 ID 的实际状态：`repair_applied`、`repair_restored`、
`repair_not_started`、`repair_recovery_required`，缺失或 ID 不匹配返回 `repair_unknown`。
调用方必须保留未知结果的跨域事务，并比较实际系统值后再提交或恢复用户设置。

PreviewRepair 在返回计划前以 create_new + fsync 保存 not_started 收据。新预览不会覆盖其他计划的完成记录。

`RecoverRepair { plan_id }` 是显式恢复动作，等待预算为 150 秒。它只恢复同 ID 的 root 原 journal，
不接受新目标。只有 GUI/Installer 的已验证 owner 会话可调用。只读状态查询不执行恢复。
