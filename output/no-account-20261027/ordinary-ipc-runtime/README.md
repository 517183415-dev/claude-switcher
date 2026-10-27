# 普通后端与真实 WebView IPC

普通 Rust main、业务处理函数、capability 和 CSP 保持现状。只覆盖测试 frontendDist，并创建独立测试 App 包装；不启用 ipc-smoke、ipc-limits 或 ipc-business。不把测试前端称作未经修改的产品 App。

| 版本 | 业务调用成功 | 缺参拒绝 | ACL 拒绝 |
| --- | ---: | ---: | ---: |
| Public | 7 | 9 | 29（含 25 个运营命令） |
| Personal | 7 | 12 | 4 |

共 68 条实际 IPC 响应。缺参拒绝只证明参数反序列化入口，不代表业务成功。两版设置和活动导出均与合成 HOME 文件独立核对。宿主读取测试页面 JSON；这属于普通功能证据读取，不涉及无障碍专项验收。

[回执](receipt.json)包含构建命令、实际配置覆盖、源输入引用、编译及执行二进制摘要、日志摘要和停止结果。Public 的有效执行为 public-r7，复用 public-r6 二进制；Personal 为 personal-r1。输入清单按执行时原样保留，运行期间输入未变。最终只补充执行二进制摘要字段，不重复运行有效业务检查。原始日志、App、合成 HOME 留在本地，不提交。

7 项 Python 校验测试、1 项 Node 报告回归通过。前五次 Public 尝试未取得 HTTP 回执，不能计为通过，也没有确认 HTTP 失败根因。public-r6 取得页面报告后，发现夹具中间目录为 755，生产业务正确拒绝写入。夹具改为逐层 700，并补目录权限回归。Node 回归防止本机报告重新依赖 HTTP 导航。

远程来源、其他窗口的动态矩阵及全部业务成功路径仍未完成。真实 Keychain、R2、传播和安装未验收。直接进程均由宿主停止，exit -15；不据此声称系统托管 WebKit 子进程全部回收。注册、部署、付款继续暂停。

复现方法见[测试入口说明](../../../apps/desktop/tests/native-ipc-policy/README.md)。
