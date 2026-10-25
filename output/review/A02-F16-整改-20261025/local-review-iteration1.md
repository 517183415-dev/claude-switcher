# A02 核心独立审查第一轮

结论：REQUEST CHANGES，1 项 P2，无 blocker/P1。只读审查；未执行测试。

含终态证据的备份恢复到无 trust 的新 Publisher 时，validate_state 隐式使用 self.verifying_key()，导致 MissingSigningKey。备份中的已验证 restored_key 没有用于终态签名验证。现有同机恢复用例未覆盖此路径。

要求：显式传入 restored_key 校验两份终态签名，验证期间不提前改变 self trust；增加全新、无公钥和 trust.json 的目标恢复回归。

其他已审路径：认证 current 优先；已提交恢复不再 CAS；明确缺失与不可用分开；终态证据与 pending 退休原子保存；rename 后错误校准；单槽保护和旧 State 兼容。

审查者：本地独立 agent /root/f01_review。此轮为修复前源码，未绑定最终冻结 hash。
