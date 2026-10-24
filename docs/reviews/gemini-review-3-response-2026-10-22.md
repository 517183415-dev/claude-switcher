# Gemini 第三轮审核：核对与处置

修订日期：2026-10-22，America/Los_Angeles。对象：用户上传的 `完成3.pdf`。

## 1. 结论与材料范围

**需要修订，现已纳入当前工程文档。后续会话从 P0 修复开始实施。**

本次 PDF 共 37 页。第 1–22 页包含旧审核与 ZIP 读取说明；第 23–37 页是补充明文材料后的新审核。

Gemini 在第 23 页明确列出：PRD v1.0、工程执行 v1.0、工作组 v1.0、主方案 v2.2、广告方案 v1.0、P0 证据与旧源码。第 24 页指出源码包在桌面 `Cargo.lock` 第 819 行截断；其后的桌面源码未完整进入该次阅读。

因此，本次记录更新为“最新方案已得到 Gemini 补审；P0 桌面源码存在明确截断”。此前针对旧 22 页 PDF 的读取判断保留为历史结论。

| 项目 | 记录 |
| --- | --- |
| 原始文件 | 用户 Downloads 中的 `# Gemini 独立方案与代码审查提示词 … 完成3.pdf` |
| PDF SHA256 | `031329729ecc65d7035820394761d30fb77a142c07428790366f0837a33129a4` |
| 页数 / 大小 | 37 页 / 2,722,811 字节 |
| 本次读取 | 37 页文本已读；新审核重点为第 23–37 页 |
| 版面限制 | 部分宽表右侧在 PDF 页面边缘截断；正文、条目及后续建议用于交叉核对 |

## 2. 十项意见逐项处置

| 编号 | 判断与证据 | 处理 | 实施/验收位置 |
| --- | --- | --- | --- |
| ISS-01 | 采纳。V-01 的 `export_all_to` 失败有日志。独立检查确认 generated 类型缺失，package scripts 未先生成 | 独立导出命令先于 check/build；固定清洁构建顺序。Vite 已通过，类型时序风险以 check/干净构建复测 | 合同 §2；P0-02；PF-01 |
| ISS-02 | 采纳。V-02 编译失败、0 测试；三个模块确为一行占位注释 | 补实质协议切片；所有层级拒绝重复字段。三组 slot 唯一，每组两个 campaign，避免误拒同位当前/下一期 | 合同 §2/3/6；P0-03 |
| ISS-03 | 采纳并补齐。Minisign 与 Ed25519 格式不同；旧方案缺衔接及安装失败隔离 | 固定元数据→同一工件字节→Minisign→安装时序；app/helper 分别绑定；持久化失败工件和组件组合；取消授权单列 | 合同 §3；UP-01/03/05；PR-05/12/14 |
| ISS-04 | 采纳并细化。双发行共用资料和服务，需要唯一拥有者及 UID 授权 | 生命周期锁→账号锁→helper 全局锁；active edition 代次及恢复；每消息代码身份、连接 EUID、激活用户/会话分别验证 | 合同 §4；P0-04/07；PR-12/13 |
| ISS-05 | 采纳。字符串是线索，实际 Keychain 依赖仍待证据 | 分为共享加密材料、账号凭据、设备状态；限定属性查询；公开报告脱敏；合成事务先推进，真实迁移等待 05b | 合同 §5；AC-06；P0-05a/b；F-08/09 |
| ISS-06 | 采纳，修订恢复算法。图片预算需分配前约束；单次 Date ±5 分钟的恢复条件不足 | 明确动画头/溢出/解码预算；无缓存时间入口、连续采样、单调时钟、系统校时与候选时间验证；TLS 保持验证 | 合同 §6.1/6.3；PR-10/14；AD-08 |
| ISS-07 | 采纳。在线 20 分钟需明确测量条件；唤醒重设周期会延迟 | 持久计划时间；按剩余时间唤醒；15+1+1+3 分钟预算；首轮成功条件；重试单列；离线暂停沿用现有缓存上限 | 合同 §6.2；KPI-04；AD-05/07/08 |
| ISS-08 | 采纳。概念图将 ASN 提示写成待确认 | 正式实现使用“13 项通过 · 1 项提示”；ASN 提示保留启动；概念图继续提供风格参考 | PRD §4/5；主方案 §2.3；F-03 |
| ISS-09 | 采纳。文件归属、跨组交接、托管范围和需求映射需补齐 | domain/ads 归 G1；六个跨组任务固定先行接口；托管同时验软件工件；补 US/AC/UP/KPI 映射及 PF-01 | 合同 §7/8/9；工程/组方案 v1.1 |
| ISS-10 | 风险成立；本轮登记过渡规则，代码保持冻结 | 开工复核旧更新停用；自动化避开旧 update/set-region/fix/安装器；Rust 使用数据配置与受签名固定 helper | 合同 §1；P0-01；迁移任务 |

## 3. 源码安全判断的校正与保留

用户关注范围为资料外发、项目方远程替换程序，以及远端输入触发本机修改。

本次核对的 29 个旧源码文件与既有审查哈希一致。静态审核未发现把账号档案、聊天、Cookie 或令牌上传给项目方的路径。实际运行载荷验收保持 NOT_RUN。

| 路径 | 已证实的能力与条件 | 当前处理 |
| --- | --- | --- |
| R-01 更新 | 上游能同时改变程序与同源 SHA 清单；清单缺条也可能通过；新脚本先执行再检查回退 | 旧定时更新停用记录开工复核；Rust 自控来源、独立签名、严格工件绑定与隔离失败版本 |
| R-02 时区输入 | 被采用的地理服务响应在满足既有条件后，可先写入可执行脚本；后续调用触发用户态执行；管理员路径另需系统授权 | Rust 配置作为数据；系统时区白名单；固定参数执行 |
| R-03 修复 plist | 用户可写 plist、服务查询失败、前置网络检查通过、系统授权等条件同时成立时，旧 fix 会提权复制并注册 | Rust 的服务描述来自已验证包；helper 固定动作、签名身份及 UID 授权 |
| 安装器重跑 | 已 disabled 的同域同标签保持禁用；旧 bootstrap 会失败并提前退出，留下部分步骤已执行 | Gemini 第 30 页已接受修正；旧安装器保持冻结 |
| 0dcloud 守护 | 旧核心查 0dcloudCore；旧 auto 守护只查 TUN | Rust GUI/guardian 共用代理适配器及证据策略 |

远端地理响应控制需具备相应服务或可信 TLS 链条件。仅掌握普通 DNS 返回不足以建立上述响应控制。R-03 还包含本地写入和授权前提。报告保持这些具体成立条件。

## 4. 独立技术核对与二次审阅

已采用 AnySearch 查找官方来源，再核对 Apple 文档、本机 SDK/手册和锁定版本官方源码。无产品测试、服务安装或网络配置改动。

| 主题 | 核对结论 | 来源 |
| --- | --- | --- |
| updater 配置 | 2.14.0 确实支持 requireSignedVersion/allowDowngrades；显式固定 true/false；自定义比较器需另验 | [官方固定 SHA 配置](https://github.com/tauri-apps/plugins-workspace/blob/44293a5722efcaf27060504e63b17aeba721b40f/plugins/updater/src/config.rs#L153-L173) |
| 工件签名 | install(bytes) 重新验 Minisign；签名 trusted comment 绑定版本；macOS 工件是 app.tar.gz | [官方验证逻辑](https://github.com/tauri-apps/plugins-workspace/blob/44293a5722efcaf27060504e63b17aeba721b40f/plugins/updater/src/updater.rs#L1893-L1967) |
| XPC | 逐消息签名验证与连接建立时 EUID 含义不同；P0 加消息 audit token 一致性验证 | [签名要求](https://developer.apple.com/documentation/xpc/xpc_connection_set_peer_code_signing_requirement(_:_:))、[EUID](https://developer.apple.com/documentation/xpc/xpc_connection_get_euid(_:)) |
| 目录交换 | 同卷 RENAME_SWAP 原子交换；多文件事务和持久化需 journal/提交点与独立证据 | 本机 renamex_np 手册；[Apple fsync](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fsync.2.html) |
| macOS UI 测试 | 当前官方支持 embedded WebDriver；测试桥接仅进入测试构建 | [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/) |
| HTTP 时间 | Date 是近似生成时间；Age 反映缓存年龄；±5 分钟属于项目策略 | [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-6.6.1)、[RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html#section-5.1) |

独立审阅进一步补齐五项：无 Age 的直源恢复、helper 元数据与失败组合、接管锁顺序、单一账号提交对象、单账号限定构建能力和发布子门槛。对应内容已进入工程合同。

最终只读复核结论为 **OKAY**。复核覆盖 PRD、工程执行、工作组、工程合同与完整执行提示词。无证书继续开发、helper 取消后重试、交换后恢复、新商家发布和单账号限定交付五条路径均已给出进入条件与结果。此结论针对方案可执行性；各运行结果继续按台账取证。

## 5. 当前交付与未完成项

现行文件为主方案 v2.3、广告方案 v1.1、PRD/工程执行/工作组/工程合同 v1.1，以及工程执行提示词和交接记录。

本次只改文档。V-01/V-02 继续保留 FAIL；真实窗口、双进程 XPC、实际托管、账号迁移、外发载荷与长期观察继续按已有状态登记。

执行会话首先修复 P0，再依据分支门槛推进。后续源码安全验收继续围绕用户指定的数据去向与远程修改能力开展。
