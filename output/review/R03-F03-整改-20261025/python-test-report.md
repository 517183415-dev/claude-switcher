# R03 Python 测试证据

- F03：17 方法，50 个显式子例 + 7 个独立场景 = 57 场景。
- 主解释器：`/opt/homebrew/opt/python@3.14/bin/python3.14`。17 项通过，4.147 秒。
- 系统 `/usr/bin/python3`（3.9）也通过17项，13.933秒。
- 有效RED：历史unquoted被旧公开verify拒绝；quoted NY改变被旧generation ownership拒绝；实际Rust旧更新经固定sed应用后被冻结旧Python verify拒绝。日志分别为 `python-red-initial.log`、`python-crosslang-pref03-red.log`。
- `python-crosslang-red.log` 的首次live执行实际exit0，不是RED。Python兼容当时已先落地。
- 真实跨语言测试只执行生产Rust planner输出的固定sed，并仅替换固定目标路径。TARGET替换表达式没有在测试中重新实现。另验证新输出canonical quotes、历史unquoted输入及同target no-op。
- generation archive和CLI保持原字节。NY变更后真实plan卸载中断，回滚两次恢复NY；没有重生成Tokyo。
- held、已发布prepared、durable restored的有效TARGET变更仍拒绝。未发布private残片可保留并废弃，恢复从可信backup创建新candidate；正向测试确认LA残片原inode/bytes不变、live恢复NY两次。原先要求所有private残片变化都拒绝的失败日志已保留，该期望经review澄清，未修改R02生产机制。
- 未运行守护脚本、真实服务、系统设置、真实账号或网络。父会话负责最终153方法及稳定hash验收。
