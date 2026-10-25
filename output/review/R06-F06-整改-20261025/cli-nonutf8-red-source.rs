use std::{io::Write, path::Path};
use switcher_core::{
    accounts::{AccountId, AccountStatus, AccountStore},
    checks, config,
    diagnostics::{self, ProxyConfig},
    lifecycle::{self, ManualUpdate},
    migration::MigrationCoordinator,
    repair_cli::{self, OutputMode, RepairCommand},
};
use switcher_domain::{
    Edition,
    checks::{CheckId, CheckReport, CheckStatus},
};

fn print_json(output: &mut impl Write, value: &impl serde::Serialize) -> Result<(), &'static str> {
    let text = serde_json::to_string(value).map_err(|_| "serialization_failed")?;
    writeln!(output, "{text}").map_err(|_| "output_failed")
}

struct Invocation {
    edition: Edition,
    mode: OutputMode,
    args: Vec<String>,
}
impl Invocation {
    fn parse(mut args: Vec<String>) -> Result<Self, String> {
        let mut mode = OutputMode::Modern;
        let edition = if args.first().is_some_and(|value| value == "--edition") {
            if args.len() < 3 {
                return Err("缺少固定发行版或命令".into());
            }
            let edition = match args[1].as_str() {
                "personal" => Edition::Personal,
                "public" => Edition::Public,
                _ => return Err("发行版只接受 personal 或 public".into()),
            };
            args.drain(..2);
            if args.first().is_some_and(|value| value == "--legacy-cli") {
                args.remove(0);
                mode = OutputMode::Legacy;
            }
            edition
        } else {
            Edition::Personal
        };
        if args.is_empty()
            || args
                .iter()
                .any(|value| matches!(value.as_str(), "--edition" | "--legacy-cli"))
        {
            return Err("invalid_command".into());
        }
        if matches!(args[0].as_str(), "check" | "checkdetail")
            && args.len() == 2
            && args[1] == "--json"
            || matches!(
                args[0].as_str(),
                "set-region"
                    | "start"
                    | "update"
                    | "diagnose-proxy"
                    | "migration-preview"
                    | "initialize-a"
                    | "associate-a"
                    | "recover"
                    | "backup"
                    | "help"
                    | "--help"
            )
        {
            mode = OutputMode::Modern;
        }
        Ok(Self {
            edition,
            mode,
            args,
        })
    }
    fn valid_business_arguments(&self) -> bool {
        match self.args[0].as_str() {
            "status" => self.args.len() == 1,
            "check" | "checkdetail" => {
                self.args.len() == 1 || self.args.len() == 2 && self.args[1] == "--json"
            }
            "switch" => self.args.len() == 2 && matches!(self.args[1].as_str(), "A" | "B"),
            "fix" => RepairCommand::parse(&self.args).is_ok(),
            _ => false,
        }
    }
}

trait CliBackend {
    fn status(&mut self) -> Result<(AccountStatus, Option<AccountId>), String>;
    async fn check(&mut self) -> Result<CheckReport, String>;
    fn save_detail(&mut self, output: &str) -> Result<(), String>;
    async fn switch(&mut self, edition: Edition, target: AccountId) -> lifecycle::LaunchOutcome;
    async fn repair(
        &mut self,
        command: RepairCommand,
        edition: Edition,
        mode: OutputMode,
        output: &mut impl Write,
    ) -> Result<i32, String>;
}
struct NativeCli;
impl CliBackend for NativeCli {
    fn status(&mut self) -> Result<(AccountStatus, Option<AccountId>), String> {
        let home = config::home_dir()?;
        AccountStore::inspect_with_recorded_account(
            &home.join(".claude-switcher"),
            &home.join("Library/Application Support/Claude"),
            unsafe { libc::geteuid() },
        )
        .map_err(|_| "account_status_unavailable".into())
    }
    async fn check(&mut self) -> Result<CheckReport, String> {
        let home = config::home_dir()?;
        Ok(checks::run_checks(config::load_read_only(&home)?).await)
    }
    fn save_detail(&mut self, output: &str) -> Result<(), String> {
        let home = config::home_dir()?;
        std::fs::create_dir_all(home.join(".claude-switcher"))
            .and_then(|()| std::fs::write(home.join(".claude-switcher/last-check.txt"), output))
            .map_err(|_| "检查报告保存失败".into())
    }
    async fn switch(&mut self, edition: Edition, target: AccountId) -> lifecycle::LaunchOutcome {
        lifecycle::switch_checked(edition, target).await
    }
    async fn repair(
        &mut self,
        command: RepairCommand,
        edition: Edition,
        mode: OutputMode,
        output: &mut impl Write,
    ) -> Result<i32, String> {
        repair_cli::run_with_output(command, edition, mode, output).await
    }
}

async fn execute_with(
    request: &Invocation,
    backend: &mut impl CliBackend,
    output: &mut impl Write,
) -> Result<i32, String> {
    let args = &request.args;
    let edition = request.edition;
    match args.first().map(String::as_str) {
        Some("check" | "checkdetail")
            if args.len() == 1 || (args.len() == 2 && args[1] == "--json") =>
        {
            let report = backend.check().await?;
            if args.len() == 2 {
                print_json(output, &report)?;
            } else if args[0] == "checkdetail" {
                let detail = legacy_details(&report);
                backend.save_detail(&detail)?;
                write!(output, "{detail}").map_err(|_| "output_failed")?;
            } else {
                let text = if request.mode == OutputMode::Legacy {
                    legacy_check_with_advisories(&report)
                } else {
                    legacy_check(&report)
                };
                writeln!(output, "{text}").map_err(|_| "output_failed")?;
            }
            Ok(if report.launch.allowed { 0 } else { 1 })
        }
        Some("diagnose-proxy") => {
            let proxy = match args.as_slice() {
                [_, flag, value] if flag == "--config" => {
                    ProxyConfig::from_private_file(Path::new(value))?
                }
                [_, flag, value] if flag == "--proxy" => ProxyConfig::parse(value)?,
                [_] => ProxyConfig::parse(
                    &std::env::var("CLAUDE_SWITCHER_PROXY_URL")
                        .map_err(|_| "explicit_proxy_required")?,
                )?,
                _ => return Err("invalid_diagnostic_arguments".into()),
            };
            let report = diagnostics::diagnose_proxy(proxy).await?;
            print_json(output, &report)?;
            Ok(if report.transport_reached { 0 } else { 1 })
        }
        Some("--help" | "help") if args.len() == 1 => {
            writeln!(output,
                "claude-switcher [--edition personal|public] check [--json] | checkdetail [--json] | status | start | switch A|B | recover | update [--auto|check|download|install]\nfix [--preview] | set-region [us|jp|tw|hk|sg|gb|de|fr|kr|au|ca] [--preview]。无国家参数时检测出口国家和时区。默认直接执行；--preview 只显示计划。diagnose-proxy --config <private-json> 保留独立诊断用途。诊断不会授予启动许可。"
            ).map_err(|_| "output_failed")?;
            Ok(0)
        }
        Some("status") if args.len() == 1 => {
            let (accounts, recorded) = backend.status()?;
            if request.mode == OutputMode::Legacy {
                let current = match recorded {
                    Some(AccountId::A) => "A",
                    Some(AccountId::B) => "B",
                    None => "none",
                };
                writeln!(
                    output,
                    "current={current} hasA={} hasB={}",
                    u8::from(accounts.a_saved),
                    u8::from(accounts.b_saved)
                )
                .map_err(|_| "output_failed")?;
                return Ok(0);
            }
            print_json(
                output,
                &serde_json::json!({
                    "accounts": {
                        "current": accounts.current,
                        "a_saved": accounts.a_saved,
                        "b_saved": accounts.b_saved,
                        "recovery_pending": accounts.recovery_pending
                    },
                    "login": "unknown"
                }),
            )?;
            Ok(0)
        }
        Some("start") if args.len() == 1 => {
            let result = lifecycle::launch_checked(edition).await;
            print_json(output, &result)?;
            Ok(if result.outcome.completed { 0 } else { 2 })
        }
        Some("update") if args.len() == 1 || args.len() == 2 => {
            let action = match args.get(1).map(String::as_str) {
                None | Some("--auto") => ManualUpdate::Check,
                Some(value) => ManualUpdate::parse(value)
                    .ok_or("更新动作只接受 --auto、check、download 或 install")?,
            };
            let result = lifecycle::update_manual(edition, action).await;
            print_json(output, &result)?;
            Ok(if result.completed { 0 } else { 2 })
        }
        Some("migration-preview") if args.len() == 1 => {
            let coordinator = MigrationCoordinator::for_current_user(edition)
                .map_err(|error| error.to_string())?;
            let result = coordinator.preview().map_err(|error| error.to_string())?;
            print_json(
                output,
                &serde_json::json!({"readiness":result.readiness,"accounts":{"current":result.accounts.current,"a_saved":result.accounts.a_saved,"b_saved":result.accounts.b_saved,"recovery_pending":result.accounts.recovery_pending}}),
            )?;
            Ok(if result.readiness.ready { 0 } else { 2 })
        }
        Some("initialize-a" | "associate-a" | "recover" | "backup") if args.len() == 1 => {
            let coordinator = MigrationCoordinator::for_current_user(edition)
                .map_err(|error| error.to_string())?;
            match args[0].as_str() {
                "initialize-a" => coordinator.initialize_a(),
                "associate-a" => coordinator.associate_a(),
                "recover" => coordinator.recover(),
                "backup" => coordinator.create_verified_backup(),
                _ => unreachable!(),
            }
            .map_err(|error| error.to_string())?;
            print_json(
                output,
                &serde_json::json!({"completed":true,"message":"账号或备份事务已完成","recovery":null}),
            )?;
            Ok(0)
        }
        Some("switch") if args.len() == 2 && matches!(args[1].as_str(), "A" | "B") => {
            let expected = if args[1] == "A" {
                switcher_core::accounts::AccountId::A
            } else {
                switcher_core::accounts::AccountId::B
            };
            let result = backend.switch(edition, expected).await;
            if request.mode == OutputMode::Legacy {
                if result.outcome.completed {
                    writeln!(output, "OK").map_err(|_| "output_failed")?;
                    return Ok(0);
                }
                writeln!(
                    output,
                    "FAIL:{}",
                    result.outcome.message.replace(['\r', '\n'], " ")
                )
                .map_err(|_| "output_failed")?;
                return Ok(1);
            }
            print_json(
                output,
                &serde_json::json!({"requested_account":args[1],"report":result.report,"outcome":result.outcome}),
            )?;
            Ok(if result.outcome.completed { 0 } else { 2 })
        }
        Some("fix" | "set-region") => {
            backend
                .repair(RepairCommand::parse(args)?, edition, request.mode, output)
                .await
        }
        _ => Err("invalid_command".into()),
    }
}

fn legacy_details(report: &CheckReport) -> String {
    let mut output = String::new();
    for id in CheckId::ALL {
        let label = match id {
            CheckId::ProxyProcess => "代理内核",
            CheckId::TunRoute => "TUN网卡",
            CheckId::EgressRegion => "出口IP",
            CheckId::IpContinuity => "IP一致",
            CheckId::Timezone => "时区",
            CheckId::Locale => "地区",
            CheckId::Ipv6Policy => "IPv6",
            CheckId::DnsRoute => "DNS",
            CheckId::StunPath => "WebRTC",
            CheckId::Guardian => "保镖",
            CheckId::ServiceRoutes => "规则置顶",
            CheckId::ClientLanguage => "客户端语言",
            CheckId::AsnInfo => "ASN属性",
            CheckId::ServiceTransport => "生态可达性",
        };
        let (status, reason) = match report.checks.iter().find(|check| check.id == id) {
            Some(check) => (
                match check.status {
                    CheckStatus::Pass => "ok",
                    CheckStatus::Warning => "warn",
                    _ => "fail",
                },
                check.reason.replace(['\r', '\n'], " "),
            ),
            None => ("fail", "缺少检查结果".into()),
        };
        output.push_str(&format!("{label}={status}:{reason}\n"));
    }
    output
}

fn legacy_check(report: &CheckReport) -> String {
    if report.launch.allowed {
        "OK".into()
    } else {
        format!("FAIL:{}", report.launch.reasons.join(";"))
    }
}

fn legacy_check_with_advisories(report: &CheckReport) -> String {
    if !report.launch.allowed {
        return format!(
            "FAIL:{}",
            report.launch.reasons.join(";").replace(['\r', '\n'], " ")
        );
    }
    // CheckReport has no structured address. Reasons are display text, not an IP source.
    let warnings = report
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::Warning)
        .map(|check| check.reason.replace(['\r', '\n'], " "))
        .collect::<Vec<_>>();
    if warnings.is_empty() {
        "OK:unavailable".into()
    } else {
        format!("OK:unavailable|{}", warnings.join(";"))
    }
}

fn finish_result(
    result: Result<i32, String>,
    mode: OutputMode,
    arguments: bool,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> i32 {
    match result {
        Ok(code) => code,
        Err(error) => {
            if mode == OutputMode::Legacy {
                let error = error.replace(['\r', '\n'], " ");
                if arguments {
                    let _ = writeln!(errors, "{error}");
                    return 2;
                }
                let _ = writeln!(output, "FAIL:{error}");
                return 1;
            }
            let _ = print_json(
                output,
                &serde_json::json!({"schema_version":1,"error":error,"recovery":"按错误提示检查配置和命令；账号事务中断时保留资料并执行 recover"}),
            );
            2
        }
    }
}

async fn run_cli(
    args: Vec<String>,
    backend: &mut impl CliBackend,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> i32 {
    let requested_mode = if args.iter().any(|value| value == "--legacy-cli") {
        OutputMode::Legacy
    } else {
        OutputMode::Modern
    };
    let request = match Invocation::parse(args) {
        Ok(request) => request,
        Err(error) => return finish_result(Err(error), requested_mode, true, output, errors),
    };
    let result = execute_with(&request, backend, output).await;
    finish_result(
        result,
        request.mode,
        !request.valid_business_arguments(),
        output,
        errors,
    )
}

async fn run_raw_cli(
    args: Vec<std::ffi::OsString>,
    backend: &mut impl CliBackend,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> i32 {
    let args = args
        .into_iter()
        .map(|s| s.into_string().map_err(|_| "invalid_command".to_owned()))
        .collect::<Result<Vec<_>, _>>();
    match args {
        Ok(args) => run_cli(args, backend, output, errors).await,
        Err(error) => finish_result(Err(error), OutputMode::Modern, true, output, errors),
    }
}

#[tokio::main]
async fn main() {
    let args = std::env::args_os().skip(1).collect();
    let mut output = std::io::stdout();
    let mut errors = std::io::stderr();
    let code = run_raw_cli(args, &mut NativeCli, &mut output, &mut errors).await;
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    struct F06Cli {
        accounts: AccountStatus,
        recorded: Option<AccountId>,
        report: CheckReport,
        completed: bool,
        message: &'static str,
        fail: bool,
        calls: Vec<String>,
    }
    impl Default for F06Cli {
        fn default() -> Self {
            Self {
                accounts: AccountStatus { current: Some(AccountId::A), a_saved: false, b_saved: false, recovery_pending: false },
                recorded: None,
                report: serde_json::from_value(serde_json::json!({"schema_version":1,"observed_at":1,"completed_at":2,"duration_ms":1,"network_generation":"synthetic","checks":[],"summary":{"passed":14,"advisory":0,"blocking":0},"launch":{"allowed":true,"reasons":[]}})).unwrap(),
                completed: true, message: "synthetic outcome", fail: false, calls: vec![],
            }
        }
    }
    impl CliBackend for F06Cli {
        fn status(&mut self) -> Result<(AccountStatus, Option<AccountId>), String> {
            self.calls.push("status".into());
            if self.fail {
                Err("synthetic_status_error".into())
            } else {
                Ok((self.accounts.clone(), self.recorded))
            }
        }
        async fn check(&mut self) -> Result<CheckReport, String> {
            self.calls.push("check".into());
            if self.fail {
                Err("synthetic_check_error".into())
            } else {
                Ok(self.report.clone())
            }
        }
        fn save_detail(&mut self, _: &str) -> Result<(), String> {
            self.calls.push("save_detail".into());
            Ok(())
        }
        async fn switch(
            &mut self,
            edition: Edition,
            target: AccountId,
        ) -> lifecycle::LaunchOutcome {
            self.calls.push(format!("switch:{edition:?}:{target:?}"));
            lifecycle::LaunchOutcome {
                report: Some(self.report.clone()),
                outcome: switcher_domain::desktop::OperationOutcome {
                    completed: self.completed,
                    message: self.message.into(),
                    recovery: None,
                },
            }
        }
        async fn repair(
            &mut self,
            command: RepairCommand,
            _: Edition,
            mode: OutputMode,
            output: &mut impl Write,
        ) -> Result<i32, String> {
            self.calls.push(format!("repair:{command:?}:{mode:?}"));
            print_json(output, &serde_json::json!({"synthetic_repair":true}))?;
            Ok(0)
        }
    }
    async fn f06_capture(name: &str, args: &[&str], fake: &mut F06Cli) -> (String, String, i32) {
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let code = run_cli(
            args.iter().map(|s| (*s).into()).collect(),
            fake,
            &mut output,
            &mut errors,
        )
        .await;
        let stdout = String::from_utf8(output).unwrap();
        let stderr = String::from_utf8(errors).unwrap();
        println!(
            "F06_CLI_RECEIPT {}",
            serde_json::json!({"case":name,"args":args,"stdout":stdout,"stderr":stderr,"code":code,"calls":fake.calls})
        );
        (stdout, stderr, code)
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn f06_non_utf8_arguments_keep_requested_error_mode() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let cases: [(&str, &[&str], bool); 5] = [
            ("nonutf8-modern", &["status"], false),
            ("nonutf8-modern-json", &["check", "--json"], false),
            ("nonutf8-legacy", &["--edition", "personal", "--legacy-cli", "status"], true),
            ("nonutf8-misplaced-legacy", &["status", "--legacy-cli"], true),
            ("nonutf8-duplicate-legacy", &["--edition", "public", "--legacy-cli", "--legacy-cli", "status"], true),
        ];
        let mut observed = Vec::new();
        for (name, prefix, legacy) in cases {
            let mut args: Vec<std::ffi::OsString> = prefix.iter().map(|s| (*s).into()).collect();
            args.push(std::ffi::OsString::from_vec(vec![0xff]));
            let raw_args: Vec<Vec<u8>> = args.iter().map(|s| s.as_bytes().to_vec()).collect();
            let mut fake = F06Cli::default();
            let mut output = Vec::new();
            let mut errors = Vec::new();
            let code = run_raw_cli(args, &mut fake, &mut output, &mut errors).await;
            let stdout = String::from_utf8(output).unwrap();
            let stderr = String::from_utf8(errors).unwrap();
            println!("F06_CLI_RECEIPT {}", serde_json::json!({"case":name,"raw_args":raw_args,"stdout":stdout,"stderr":stderr,"code":code,"calls":fake.calls}));
            observed.push((name, legacy, stdout, stderr, code, fake.calls));
        }
        for (name, legacy, stdout, stderr, code, calls) in observed {
            assert!(calls.is_empty(), "{name}: backend must not run");
            let expected = if legacy {
                (String::new(), "invalid_command\n".to_owned(), 2)
            } else {
                (format!("{}\n", serde_json::json!({"schema_version":1,"error":"invalid_command","recovery":"按错误提示检查配置和命令；账号事务中断时保留资料并执行 recover"})), String::new(), 2)
            };
            assert_eq!((stdout, stderr, code), expected, "{name}");
        }
    }

    #[tokio::test]
    async fn f06_legacy_status_uses_recorded_owner_not_display_default() {
        for (recorded, a_saved, b_saved, expected) in [
            (None, false, false, "current=none hasA=0 hasB=0\n"),
            (Some(AccountId::A), true, false, "current=A hasA=1 hasB=0\n"),
            (Some(AccountId::B), true, true, "current=B hasA=1 hasB=1\n"),
        ] {
            let mut fake = F06Cli {
                recorded,
                ..Default::default()
            };
            fake.accounts.a_saved = a_saved;
            fake.accounts.b_saved = b_saved;
            let got = f06_capture(
                &format!("legacy-status-{recorded:?}"),
                &["--edition", "public", "--legacy-cli", "status"],
                &mut fake,
            )
            .await;
            assert_eq!(got, (expected.into(), String::new(), 0));
            assert_eq!(fake.calls, ["status"]);
        }
    }
    #[tokio::test]
    async fn f06_legacy_check_allowed_uses_unavailable_and_display_advisory() {
        for warning in [None, Some("synthetic advisory 198.18.0.1\nsecond line")] {
            let mut fake = F06Cli::default();
            if let Some(reason) = warning {
                fake.report
                    .checks
                    .push(switcher_domain::checks::CheckResult {
                        id: CheckId::AsnInfo,
                        status: CheckStatus::Warning,
                        reason: reason.into(),
                        observed_at: 1,
                        duration_ms: 1,
                        valid_until: 2,
                        network_generation: "synthetic".into(),
                        evidence: vec![],
                    });
                fake.report.summary.advisory = 1;
            }
            let got = f06_capture(
                "legacy-check-allowed",
                &["--edition", "personal", "--legacy-cli", "check"],
                &mut fake,
            )
            .await;
            let expected = if warning.is_some() {
                "OK:unavailable|synthetic advisory 198.18.0.1 second line\n"
            } else {
                "OK:unavailable\n"
            };
            assert_eq!(got, (expected.into(), String::new(), 0));
            assert_eq!(fake.calls, ["check"]);
        }
    }
    #[tokio::test]
    async fn f06_legacy_check_blocking_and_runtime_errors() {
        let mut fake = F06Cli::default();
        fake.report.launch.allowed = false;
        fake.report.launch.reasons = vec!["synthetic gate".into()];
        assert_eq!(
            f06_capture(
                "legacy-check-blocked",
                &["--edition", "personal", "--legacy-cli", "check"],
                &mut fake
            )
            .await,
            ("FAIL:synthetic gate\n".into(), String::new(), 1)
        );
        for command in ["check", "status"] {
            let mut fake = F06Cli {
                fail: true,
                ..Default::default()
            };
            assert_eq!(
                f06_capture(
                    "legacy-runtime-error",
                    &["--edition", "personal", "--legacy-cli", command],
                    &mut fake
                )
                .await,
                (
                    format!("FAIL:synthetic_{command}_error\n"),
                    String::new(),
                    1
                )
            );
        }
    }
    #[tokio::test]
    async fn f06_legacy_switch_outcome_and_exit_codes() {
        for (completed, message) in [
            (true, "synthetic complete"),
            (false, "synthetic gate"),
            (false, "synthetic selection"),
            (false, "synthetic launch"),
        ] {
            let mut fake = F06Cli {
                completed,
                message,
                ..Default::default()
            };
            let expected = if completed {
                "OK\n".into()
            } else {
                format!("FAIL:{message}\n")
            };
            assert_eq!(
                f06_capture(
                    message,
                    &["--edition", "public", "--legacy-cli", "switch", "B"],
                    &mut fake
                )
                .await,
                (expected, String::new(), if completed { 0 } else { 1 })
            );
            assert_eq!(fake.calls, ["switch:Public:B"]);
        }
    }
    #[tokio::test]
    async fn f06_global_flag_and_command_errors_have_zero_backend_calls() {
        for args in [
            vec!["--legacy-cli", "status"],
            vec!["--edition", "personal", "status", "--legacy-cli"],
            vec![
                "--edition",
                "personal",
                "--legacy-cli",
                "--legacy-cli",
                "status",
            ],
            vec!["--edition", "personal", "--legacy-cli", "switch", "C"],
            vec!["--edition", "personal", "--legacy-cli", "status", "--json"],
            vec!["--edition", "personal", "--legacy-cli", "fix", "--json"],
            vec![
                "--edition",
                "personal",
                "--legacy-cli",
                "switch",
                "A",
                "--json",
            ],
            vec![
                "--edition",
                "personal",
                "--legacy-cli",
                "check",
                "--json",
                "extra",
            ],
            vec!["--edition", "personal", "--legacy-cli", "--unknown"],
        ] {
            let mut fake = F06Cli::default();
            assert_eq!(
                f06_capture("legacy-invalid-args", &args, &mut fake).await,
                (String::new(), "invalid_command\n".into(), 2)
            );
            assert!(fake.calls.is_empty());
        }
    }
    #[tokio::test]
    async fn f06_modern_defaults_json_override_and_set_region_stay_unchanged() {
        let mut fake = F06Cli::default();
        assert_eq!(
            f06_capture("modern-bare-check", &["check"], &mut fake).await,
            ("OK\n".into(), String::new(), 0)
        );
        let mut fake = F06Cli::default();
        let (out, err, code) = f06_capture("modern-status", &["status"], &mut fake).await;
        assert_eq!(
            out,
            "{\"accounts\":{\"a_saved\":false,\"b_saved\":false,\"current\":\"A\",\"recovery_pending\":false},\"login\":\"unknown\"}\n"
        );
        assert_eq!((err, code), (String::new(), 0));
        for command in ["check", "checkdetail"] {
            let mut fake = F06Cli::default();
            let expected = format!("{}\n", serde_json::to_string(&fake.report).unwrap());
            assert_eq!(
                f06_capture(
                    "legacy-json-override",
                    &["--edition", "personal", "--legacy-cli", command, "--json"],
                    &mut fake
                )
                .await,
                (expected, String::new(), 0)
            );
            assert_eq!(fake.calls, ["check"]);
        }
        for legacy in [false, true] {
            let mut fake = F06Cli::default();
            let args = if legacy {
                vec!["--edition", "personal", "--legacy-cli", "checkdetail"]
            } else {
                vec!["checkdetail"]
            };
            let expected = legacy_details(&fake.report);
            assert_eq!(
                f06_capture("bare-checkdetail", &args, &mut fake).await,
                (expected, String::new(), 0)
            );
            assert_eq!(fake.calls, ["check", "save_detail"]);
        }
        for completed in [false, true] {
            let mut fake = F06Cli {
                completed,
                ..Default::default()
            };
            let expected = format!(
                "{}\n",
                serde_json::json!({"requested_account":"A","report":fake.report,"outcome":{"completed":completed,"message":"synthetic outcome","recovery":null}})
            );
            let (out, err, code) = f06_capture("modern-switch", &["switch", "A"], &mut fake).await;
            assert_eq!(out, expected);
            let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(parsed["requested_account"], "A");
            assert_eq!(parsed["outcome"]["completed"], completed);
            assert_eq!((err, code), (String::new(), if completed { 0 } else { 2 }));
        }
        let mut fake = F06Cli::default();
        assert_eq!(
            f06_capture(
                "legacy-set-region-stays-modern",
                &[
                    "--edition",
                    "personal",
                    "--legacy-cli",
                    "set-region",
                    "us",
                    "--preview"
                ],
                &mut fake
            )
            .await,
            ("{\"synthetic_repair\":true}\n".into(), String::new(), 0)
        );
        assert_eq!(
            fake.calls,
            ["repair:SetRegion { preset: Us, apply: false }:Modern"]
        );
        let mut fake = F06Cli {
            fail: true,
            ..Default::default()
        };
        let (out, err, code) = f06_capture("modern-runtime-error", &["check"], &mut fake).await;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&out).unwrap()["error"],
            "synthetic_check_error"
        );
        assert_eq!((err, code), (String::new(), 2));
    }
    #[tokio::test]
    async fn f06_public_inspection_is_read_only_for_unknown_and_recorded_accounts() {
        use std::os::unix::fs::PermissionsExt;
        let parent = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "f06-status-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&parent).unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let _cleanup = Cleanup(parent.clone());
        let root = parent.join("state");
        let active = parent.join("active");
        for (name, recorded) in [
            ("empty", None),
            ("active-unrecorded", None),
            ("recorded-A", Some(AccountId::A)),
            ("recorded-B", Some(AccountId::B)),
        ] {
            if name != "empty" && !active.exists() {
                std::fs::create_dir(&active).unwrap();
                std::fs::write(active.join("synthetic"), b"not an account").unwrap();
            }
            if name != "empty" && !root.exists() {
                std::fs::create_dir(&root).unwrap();
                std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            if let Some(recorded) = recorded {
                std::fs::write(root.join("current"), format!("{recorded:?}\n")).unwrap();
                std::fs::set_permissions(
                    root.join("current"),
                    std::fs::Permissions::from_mode(0o600),
                )
                .unwrap();
            }
            let before = (
                root.exists(),
                active.exists(),
                std::fs::read(root.join("current")).ok(),
                std::fs::read(active.join("synthetic")).ok(),
            );
            let (status, actual) =
                AccountStore::inspect_with_recorded_account(&root, &active, unsafe {
                    libc::geteuid()
                })
                .unwrap();
            assert_eq!(actual, recorded);
            assert_eq!(status.current, recorded.or(Some(AccountId::A)));
            let mut fake = F06Cli {
                accounts: status,
                recorded: actual,
                ..Default::default()
            };
            let owner = match actual {
                Some(AccountId::A) => "A",
                Some(AccountId::B) => "B",
                None => "none",
            };
            assert_eq!(
                f06_capture(
                    name,
                    &["--edition", "personal", "--legacy-cli", "status"],
                    &mut fake
                )
                .await,
                (format!("current={owner} hasA=0 hasB=0\n"), String::new(), 0)
            );
            assert_eq!(
                before,
                (
                    root.exists(),
                    active.exists(),
                    std::fs::read(root.join("current")).ok(),
                    std::fs::read(active.join("synthetic")).ok()
                )
            );
            assert!(!root.join("v2").exists());
            assert!(!root.join("accounts").exists());
        }
    }

    #[test]
    fn legacy_report_keeps_fourteen_unique_rows_and_failures() {
        let mut report: CheckReport = serde_json::from_value(serde_json::json!({
            "schema_version":1,"observed_at":1,"completed_at":2,"duration_ms":1,
            "network_generation":"legacy","checks":[],
            "summary":{"passed":0,"advisory":0,"blocking":14},
            "launch":{"allowed":false,"reasons":["出口地区不符"]}
        }))
        .unwrap();
        let output = legacy_details(&report);
        assert_eq!(output.lines().count(), 14);
        assert!(output.contains("出口IP=fail:"));
        assert!(output.contains("生态可达性=fail:"));
        assert_eq!(legacy_check(&report), "FAIL:出口地区不符");
        report.launch.allowed = true;
        assert_eq!(legacy_check(&report), "OK");
    }
}
