# R08 / F07 implementation handoff — NOT_ACCEPTED

本执行轮发生 **no-install/no-download boundary violation**。不能用本记录宣称 R08 验收通过。父会话已接管最终离线验证；两个业务文件保持冻结。

## 变更与冻结

- `packaging/macos/local_install.py` SHA256：`9ea2fac8262cf43b22b9a6989c4511f531158aa0a49638880fbb5a1b3207804f`。
- `packaging/macos/test_local_install.py` SHA256：`c468148ac433b4054c66f71f8ae24b4b9a83ee25674f0e3c408c3162a63b5bd0`。
- `plan:119`、`installation_settings:1180`、`main:2162` 显式传递 mode/group，最终选择继续进入原有 install_literals 和 digest。完整已有配置不探测；相同选择接受，冲突拒绝。
- `detect_proxy:1140` 对未知/多客户端和 0/N 组要求选择；单组自动接受。只发现 Clash 目录和 Shadowrocket 注册，不新增客户端探测器。组名保留 Unicode/空格，拒绝控制字符；嵌套代理列表不截断所在 group。
- `post_install_handoff:2130` 只由 committed install/replace/finalize 在锁外调用。通过当前 record、既有 installed_cli_plan proof 与返回 stage 绑定 generation/owner/edition，使用固定已安装 App、owner uid/gid、清空环境、固定 HOME/PATH 和无 shell 的 Popen。普通启动没有 health 参数。响应区分 requested/failed 与环境 NOT_RUN；不修改 durable archive。`launch_health`、`finalize` 均保持原源码。

## RED 与行为回执

`f07-red.json/.stdout/.stderr` 和 `f07-red-*.py` 记录原生产 hash 下的 3 个真实行为 FAIL，0 ERROR。多组/未知客户端均被接受且 locks/stopped/transact 各 1 次；真实 main/transact/finalize 已 durable committed，但普通 GUI Popen 为 0。

`f07-targeted.stdout/.stderr`：15 个方法通过。覆盖 0/1/N 组、Unicode/空格、未知客户端显式六种模式、多客户端显式选择、控制字符/非法 mode、legacy/v2 配置零探测与冲突拒绝、真实 main plan/install digest 绑定。更改/缺少选择在 stopped/locks/transact 前拒绝。

真实隔离事务的 install/replace/finalize：`lock enter → component committed → lock exit → normal GUI launch`，各请求 1 次，health launcher 0 次。explicit finalize 使用事务 public edition，不采用 argparse 默认 personal。commit 0 次。真实 main 分支的 recover/restored、recover/committed、uninstall/uninstalled 及 finalize 的 uninstalled/restored/backup_aborted 回执均为 0 次。后者以合成 terminal 结果测试路由，不声称重新验证完整 recovery/uninstall 实现。

Popen 成功仅 `requested`；合成 OSError 为 `failed/manual_open_required`，均 `environment_verified=false`、`environment_status=NOT_RUN`。比较 commit 后 SYSTEM 文件全部字节，handoff 前后相同。Popen argv/kwargs 精确检查 uid=501、gid=502、隔离 HOME、固定 PATH、固定 App、无 shell/health nonce。LocalInstallTests 默认拦住 launchctl Popen，保留既有临时 fake-bash shim 的真实隔离执行。

## 全模块记录与旧测试调整

首次完整模块：173 tests，1 failure、3 errors，exit 1。原始 `f07-full-initial.stdout/.stderr` 保留。四项都是批准行为或调用签名对应的旧测试调整：

1. 未知客户端静默 auto 的旧断言改为缺选择拒绝、显式 auto 接受。
2. indentless YAML 无 select 组的 GLOBAL 旧断言改为 selection required；仍检查 next top-level mapping 边界。
3. 静默第一组与禁止 scutil 的旧断言改为显式 mode/group；继续验证 `$` 不被执行。
4. replace 的 `make_plan(*args)` 夹具改为接收新的关键词参数；原 digest/edition 断言保留。

另一个新测试夹具最初用 shlex.quote 生成了不加引号的地区 literal，导致既有 legacy parser 忽略它。该夹具已改为明确带引号的 literal；没有修改受保护 renderer/parser 来迁就夹具。

第二次完整模块：175 tests，4 个失败子场景，exit 1；涉及既有 F03 bridge 的 2 个方法。执行器错误地只设置临时 HOME，没有同时固定现有 CARGO_HOME/RUSTUP_HOME，导致 cargo --offline 前的 rustup bootstrap。**这次执行 NOT_ACCEPTED，不得简化为“产品无回归”或“无网络”。**

## 边界违规证据

原始文件为本目录 `f07-full.stdout`、`f07-full.stderr`、`f07-validation.json`，未覆盖删除。

`f07-full.stderr:196`：`syncing channel updates for 1.96.0-aarch64-apple-darwin`；`:198`：`downloading 5 components`；`:199`：离线缺 serde index。没有更细的下载/安装完成记录，不能推断实际字节量。`f07-boundary-violation.json` 保存准确日志行号与原始文件 hash。

runner session 39402 已结束。TemporaryDirectory 上下文正常退出后，元数据检查确认 `f07-verification-home-*` 残留 0，cargo/rustup/rustc 进程 0。没有删除共享缓存或修复工具链。之后停止全部完整测试与工具链命令，交由父会话处理。真实产品网络/GUI/native IPC/安装/服务操作未作为本轮测试动作执行，但不能因此掩盖工具链下载边界违规。

## 保护与已有 GUI 调用证据

`f07-protected.json`：30 个既有函数 AST 和函数源码字节全部未变；其余 14 个既有文件 hash 全部与 baseline-sha256.json 相同。`f07-source-diff.patch` 为本轮生产差异，`f07-final-*.py` 为冻结候选。两个源码在定向/全模块命令前后的 hash 一致，详见 validation.json。

首次 GUI 的既有调用锚点来自父会话 `first-gui-call-evidence.md`，并绑定当前未变 R07 文件 hash：App.svelte:198/210 `bootstrap → runtime_info → refresh/workspace_snapshot → runChecks/check_environment`；RegionSettings 的已接受 `preview_repair → execute_repair → completed=true → check_environment`。本轮不复制修复器、不修改 R07。真实 GUI 自动检查与环境全绿仍 **NOT_RUN**。

实际命令：`/opt/homebrew/bin/python3 -B -m unittest discover -s packaging/macos -p test_local_install.py -k f07 -v`；完整模块去掉 `-k f07`。两者第二轮使用临时 HOME，确切命令、耗时、exit 与源码 hash 见 `f07-validation.json`。全量失败后未运行预定 git diff --check；该项交父会话统一验证。


## 独立审查后的窄注释修复

独立审查发现生产缺陷：合法 `type: select # 注释` 被漏掉，以及 `proxy-groups: # 注释` 令整个区块被漏掉。后者把已存在候选当成零候选，错误允许 Missing 显式组。这与前述四个旧测试适配不同，属于真实实现缺陷。

生产修改前分别保存 `f07-type-comment-red.*`（2 方法、4 个行为失败）和 `f07-header-comment-red.*`（1 方法、4 个行为失败）。type RED 的实际 main 无选择错误进入 locks/stopped/transact；header RED 错误接受 Missing。

仅修改 header/type 两个正则：行内空白、合法空白分隔的尾注释、CRLF；type 为精确 select 或成对单/双引号 select。不扩展为完整 YAML 引擎，不改变名称合同。`select#suffix` 不作为 select；`url-test # select` 也不计入。

`f07-comment-green.json/.stdout/.stderr` 记录最新冻结源：18 个 f07 方法全部通过，exit 0，纯 Python 定向。没有再次运行全模块、Cargo 或 Rustup。父会话已另行完成旧冻结源的 15 定向/175 全量/whitespace 验证，结果存 pre-comment-fix-validation/；最新冻结源的最终全量验收仍归父会话。本轮先前边界违规记录保留，不能被后续 GREEN 擦除。

最新 f07-protected.json 再次确认 30 旧函数 AST/源码字节与 14 个既有文件 hash 全部未变。最新两源码快照与生产差异已更新；后续没有源码写入。
