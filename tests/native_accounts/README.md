# 独立磁盘卷上的账号事务测试

仅限 macOS。需要已安装的 Rust 工具链、Xcode/命令行工具及系统 `hdiutil`。不需要 Claude 账号，不修改安装、系统代理、Keychain 或现有账号资料。

```sh
python3 tests/native_accounts/run.py --output output/native-account-volume-001
```

输出目录必须不存在。运行器创建 128 MiB 临时 APFS 磁盘映像，用隐藏挂载点运行四项合成资料测试，再卸载自己创建的映像。两个磁盘卷测试默认被 Rust 测试框架忽略，必须由此运行器提供独立卷；它们不会静默退化成同卷测试。

跨卷测试覆盖 active 在临时卷和归档在临时卷两种方向。空间不足测试只填充临时卷中的新建文件；观察实际 ENOSPC 后释放 4 MiB，再复制 32 MiB 的确定性合成数据。失败后核对 active、A/B 归档、状态、current 和事务清理。

仅重跑失败用例时，可用 `--filter f05_native_full_destination_preserves_both_accounts_and_state`。每次使用新的输出目录。运行器保存命令、源码摘要、日志和卸载回执。清理只选择 `hdiutil info` 中映像路径完全匹配的设备，不选择名称相似的其他映像。

运行器单元检查：`python3 -m unittest discover -s tests/native_accounts -p test_run.py -v`。
