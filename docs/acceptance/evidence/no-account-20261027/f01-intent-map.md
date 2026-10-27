# F-01：旧版 21 项意图与现行 Rust 用例

核对日期：2026-10-27。旧测试的实际数量为 21 项：代理诊断 13 项，切换安全 8 项。下表逐项列出当前 Rust 用例。P 编号对应 [覆盖索引](coverage-map.json)。

这些用例的通过记录来自 [历史隔离测试日志](../../../../output/no-account-20261027/storage-final-verify/third/tests.log) 和 [回执](../../../../output/no-account-20261027/storage-final-verify/third/final-receipt.json)。已核对日志 SHA-256 与回执一致，24 个命名测试均有 `ok` 记录。没有重新运行有效检查。

历史测试候选的 Git HEAD 为 `b682e0b398bcfd5e0cbd40ab999655a8cbc20996`。当前 8 个测试所在文件与该 Git HEAD 对比，7 个相同；`network.rs` 仅增加受测试配置限制的 SOCKS 测试模块声明。其他共享源码和依赖此后有变化。因此历史结果不能冒充当前完整源码的全量验收。

| 编号 | 旧测试意图 | 当前 Rust 用例 |
| --- | --- | --- |
| P001 | `test_401_and_403_are_transport_only_not_launch_approval`：401/403 仅表示传输或访问状态 | T01, T02 |
| P002 | `test_argument_error_does_not_echo_unknown_secret`：参数错误不回显秘密 | T03 |
| P003 | `test_bad_metrics_and_tls_failure_never_pass`：TLS 失败不得判为通过 | T04 |
| P004 | `test_existing_status_command_retains_its_behavior`：status 只读 | T05 |
| P005 | `test_explicit_environment_proxy_is_supported`：允许显式环境代理 | T02 |
| P006 | `test_installed_single_script_needs_no_extra_helpers`：无需额外 shell 工具 | T02 |
| P007 | `test_invalid_config_reports_no_private_path_or_contents`：无效配置不泄漏路径或内容 | T03 |
| P008 | `test_invalid_proxy_never_probes_or_echoes_secrets`：无效代理先拒绝，且不泄漏 | T03, T06 |
| P009 | `test_missing_curl_reports_unknown_without_fallback`：不依赖 curl，也不直连回退 | T02, T07 |
| P010 | `test_no_implicit_system_proxy_or_direct_fallback`：不能隐式使用系统代理或回退直连 | T03, T07 |
| P011 | `test_private_config_and_credentials_do_not_enter_output_or_argv`：私有配置和凭据不进入输出或子进程 | T02, T08 |
| P012 | `test_socks_ipv6_does_not_require_an_http_connect_status`：SOCKS IPv6 不要求 HTTP CONNECT | T09 |
| P013 | `test_timeouts_stay_unknown`：传输超时和无有效 STUN 响应不能通过 | T04, T10, T11 |
| P014 | `test_ip_sb_fallback_supplies_country_and_timezone`：第三个地理来源提供国家与时区 | T12, T13 |
| P015 | `test_missing_check_is_blocked`：缺少或重复检查阻断启动 | T14 |
| P016 | `test_warning_is_allowed_only_for_ip_history_and_asn`：仅指定的两项检查可用警告放行 | T15 |
| P017 | `test_warnings_are_allowed_after_all_fourteen_checks_run`：全部 14 项齐全后才可放行 | T14, T15 |
| P018 | `test_auto_proxy_detects_odcloud_core`：自动代理识别 0dcloud | T16 |
| P019 | `test_blocks_account_switch_when_preflight_has_failed_checks`：首次预检失败不切换，不启动 | T17, T18, T19 |
| P020 | `test_fix_does_not_change_system_settings_without_a_tunnel`：无 TUN 时不改系统设置 | T20 |
| P021 | `test_rejects_targets_other_than_a_or_b_before_starting_claude`：拒绝非 A/B 目标 | T21 |

## 用例索引

- T01：[源码](../../../../crates/switcher-core/src/network.rs)：`network::tests::authentication_and_access_do_not_assert_account_usability`。
- T02：[源码](../../../../crates/switcher-core/tests/checks_cli.rs)：`explicit_environment_and_private_file_need_no_curl_or_helpers_and_never_grant_launch`。
- T03：[源码](../../../../crates/switcher-core/tests/checks_cli.rs)：`invalid_arguments_and_config_do_not_echo_paths_or_secrets`。
- T04：[源码](../../../../crates/switcher-core/src/network.rs)：`network::tests::explicit_proxy_authentication_tls_and_timeout_are_distinct`。
- T05：[源码](../../../../crates/switcher-core/tests/checks_cli.rs)：`status_is_readonly_reports_login_unknown_and_rejects_arguments`。
- T06：[源码](../../../../crates/switcher-core/src/diagnostics.rs)：`diagnostics::tests::explicit_proxy_validation_and_socks_dns_are_private`。
- T07：[源码](../../../../crates/switcher-core/src/diagnostics.rs)：`diagnostics::tests::dead_proxy_does_not_fallback_or_modify_state`。
- T08：[源码](../../../../crates/switcher-core/src/diagnostics.rs)：`diagnostics::tests::redaction_is_shared_and_idempotent`。
- T09：[源码](../../../../crates/switcher-core/src/network.rs)：`network::tests::socks_ipv6_uses_remote_domain_dns_without_http_connect_and_classifies_authentication`。
- T10：[源码](../../../../crates/switcher-core/src/checks.rs)：`checks::tests::stun_without_valid_responses_is_unknown_with_or_without_https_egress`。
- T11：[源码](../../../../crates/switcher-core/src/checks.rs)：`checks::tests::stun_partial_response_matches_egress_but_mismatch_blocks`。
- T12：[源码](../../../../crates/switcher-core/src/network.rs)：`network::tests::geo_ip_sb_fallback_and_failure_are_distinct`。
- T13：[源码](../../../../crates/switcher-core/tests/e11_privacy.rs)：`geo_production_lookup_reaches_third_ip_sb_source_after_failures`。
- T14：[源码](../../../../crates/switcher-core/src/checks.rs)：`checks::tests::gate_requires_complete_unique_results_and_fresh_evidence`。
- T15：[源码](../../../../crates/switcher-core/src/checks.rs)：`checks::tests::only_two_advisory_warnings_can_pass_the_gate`。
- T16：[源码](../../../../crates/switcher-core/src/macos.rs)：`macos::tests::automatic_proxy_adapter_includes_odcloud_and_clash_verge`。
- T17：[源码](../../../../crates/switcher-core/src/lifecycle.rs)：`lifecycle::tests::preflight_failure_does_not_quit_switch_or_open`。
- T18：[源码](../../../../crates/switcher-core/src/lifecycle.rs)：`lifecycle::tests::first_b_checks_before_selection_and_again_before_open`。
- T19：[源码](../../../../crates/switcher-core/src/lifecycle.rs)：`lifecycle::tests::check_error_after_selection_preserves_target_until_fresh_launch_check`。
- T20：[源码](../../../../crates/switcher-helper/src/platform.rs)：`platform::no_tun_test_io::no_tun_apply_plan_never_reaches_system_or_journal_writes`；`platform::no_tun_test_io::no_tun_missing_boundary_fails_closed_and_unwind_resets_scope`；`platform::no_tun_test_io::no_tun_positive_control_trips_before_root_journal_io`；`platform::no_tun_test_io::no_tun_scope_is_thread_local_and_denies_unexpected_commands`。
- T21：[源码](../../../../crates/switcher-core/tests/checks_cli.rs)：`unavailable_lifecycle_commands_never_start_or_modify_accounts`。

## 语义差异与限制

- P004：现行 status 返回只读 JSON；不沿用旧文本和创建目录的副作用。
- P006/P009：Rust 不再依赖 curl；测试确认无 curl 也不会回退直连。
- P019：现行 CLI `switch` 在首次预检通过后切换，随后再次检查才启动。切换后复检失败时保留已选账号，不启动。普通 UI 的账号选择入口需要单独验证。
- P020：使用模拟系统命令调用真实 `apply_plan`；不等同真实 root 操作或系统修改。
- P021：现有 CLI 测试覆盖非法目标及隔离目录不变；其函数名保留历史命名，不能说明合法的 switch 命令不可用。
- 未使用真实 Claude 账号、真实代理或生产系统权限。完整系统、线上及长时验收仍按原门槛记录。

[E02 历史映射](../e02/旧21项测试意图映射.md)保留原样。其两个已变更的测试名及旧时序说明以本表为准。
