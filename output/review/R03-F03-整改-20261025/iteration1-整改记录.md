# R03 / F03 第 1 轮

状态：本地执行完成，ChatGPT 第 1 轮独立复核接受。单元剩余工作为零。接受结论见 `chatgpt-review-iteration1.txt`，已复核输出 12–22。真实部署层面的未运行项目仍保留。

1. Rust 校验接受当前 Python plistlib 输出和原 bundled plist 的两个完整受控制式。其余字节变体拒绝。即使服务已注册，也检查制品。通过同一 no-follow FD 读取，并验证 UID/GID、模式、文件类型、链接数和父目录。
2. Rust 时区更新使用唯一、完整、有效的 TARGET 字面量。新更新输出双引号。历史无引号且目标相同的输入保持 no-op。生产路径保持固定。
3. Python 公开验证接受上述两种 TARGET 写法，且 TARGET 必须匹配当前配置。卸载计划仅允许相对不可变代际归档的 TARGET 变化；归档目标仍须匹配归档计划。比较读取的 hash、元数据与身份必须等于已观察快照。
4. R02 的七个事务与恢复函数保持逐字相同。预览后、holding、已发布候选、已恢复目标仍比较完整指纹。未发布废弃候选的边界解释见 `prepared-边界复核.md`。
5. 最终八条命令均退出 0。Python 为 153 方法，内含 43 个 F02 和 17 个 F03 方法。F03 有 57 场景。Rust 守护 17 方法，内含 6 个 F03；地区 51 方法。定向与完整运行重叠，不累加计数。Clippy、rustfmt、diff 检查通过。

本单元只修改 `packaging/macos/local_install.py`、`packaging/macos/test_local_install.py`、`crates/switcher-core/src/guardian_repair.rs`。累积 tracked diff 为六文件，无依赖、lockfile、daemon template 或其他源文件修改。

`verification.json` 绑定 HEAD、六文件源码 hash、每条命令的前后 hash、日志、退出码与非零测试数。`python-source-boundary.json` 记录 R02 七函数未变。F01 三文件 hash 与 R03 基线一致。

有效 RED：Rust 4 方法中 3 个行为失败；Python 初始 3 方法中 2 个真实拒绝；跨语言更新另有修复前拒绝。跨语言首次 live 运行已先获得 Python 兼容，因此为 exit 0，明确不计 RED。冻结旧 Python 副本 hash 与 R02 最终源码一致，随后补得有效跨语言 RED。

所有测试使用临时文件、合成 HOME、虚拟凭据和假服务。未安装产品或依赖；未访问真实 Claude 账号；未运行实际服务、系统设置、产品网络探测或原生 GUI。断电持久性和真实部署验收未运行。
