use std::path::Path;
use switcher_core::{
    checks, config,
    diagnostics::{self, ProxyConfig},
    lifecycle::{self, ManualUpdate},
    migration::MigrationCoordinator,
    repair_cli::{self, RepairCommand},
};
use switcher_domain::{
    Edition,
    checks::{CheckId, CheckReport, CheckStatus},
};

fn print_json(value: &impl serde::Serialize) -> Result<(), &'static str> {
    let output = serde_json::to_string(value).map_err(|_| "serialization_failed")?;
    // DTO fields are constructed from fixed labels and enums, with no raw network output.
    println!("{output}");
    Ok(())
}

async fn execute() -> Result<i32, String> {
    let mut args: Vec<_> = std::env::args_os()
        .skip(1)
        .map(|s| s.into_string().map_err(|_| "invalid_command"))
        .collect::<Result<_, _>>()?;
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
        edition
    } else {
        Edition::Personal
    };
    match args.first().map(String::as_str) {
        Some("check" | "checkdetail")
            if args.len() == 1 || (args.len() == 2 && args[1] == "--json") =>
        {
            let home = config::home_dir()?;
            let config = config::load_read_only(&home)?;
            let report = checks::run_checks(config).await;
            if args.len() == 2 {
                print_json(&report)?;
            } else if args[0] == "checkdetail" {
                let output = legacy_details(&report);
                std::fs::create_dir_all(home.join(".claude-switcher"))
                    .and_then(|()| {
                        std::fs::write(home.join(".claude-switcher/last-check.txt"), &output)
                    })
                    .map_err(|_| "检查报告保存失败")?;
                print!("{output}");
            } else {
                println!("{}", legacy_check(&report));
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
            print_json(&report)?;
            Ok(if report.transport_reached { 0 } else { 1 })
        }
        Some("--help" | "help") if args.len() == 1 => {
            println!(
                "claude-switcher [--edition personal|public] check [--json] | checkdetail [--json] | status | start | switch A|B | recover | update [--auto|check|download|install]\nfix [--preview] | set-region [us|jp|tw|hk|sg|gb|de|fr|kr|au|ca] [--preview]。无国家参数时检测出口国家和时区。默认直接执行；--preview 只显示计划。diagnose-proxy --config <private-json> 保留独立诊断用途。诊断不会授予启动许可。"
            );
            Ok(0)
        }
        Some("status") if args.len() == 1 => {
            let home = config::home_dir()?;
            let accounts = switcher_core::accounts::AccountStore::inspect(
                &home.join(".claude-switcher"),
                &home.join("Library/Application Support/Claude"),
                unsafe { libc::geteuid() },
            )
            .map_err(|_| "account_status_unavailable")?;
            print_json(&serde_json::json!({
                "accounts": {
                    "current": accounts.current,
                    "a_saved": accounts.a_saved,
                    "b_saved": accounts.b_saved,
                    "recovery_pending": accounts.recovery_pending
                },
                "login": "unknown"
            }))?;
            Ok(0)
        }
        Some("start") if args.len() == 1 => {
            let result = lifecycle::launch_checked(edition).await;
            print_json(&result)?;
            Ok(if result.outcome.completed { 0 } else { 2 })
        }
        Some("update") if args.len() == 1 || args.len() == 2 => {
            let action = match args.get(1).map(String::as_str) {
                None | Some("--auto") => ManualUpdate::Check,
                Some(value) => ManualUpdate::parse(value)
                    .ok_or("更新动作只接受 --auto、check、download 或 install")?,
            };
            let result = lifecycle::update_manual(edition, action).await;
            print_json(&result)?;
            Ok(if result.completed { 0 } else { 2 })
        }
        Some("migration-preview") if args.len() == 1 => {
            let coordinator = MigrationCoordinator::for_current_user(edition)
                .map_err(|error| error.to_string())?;
            let result = coordinator.preview().map_err(|error| error.to_string())?;
            print_json(
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
            let result = lifecycle::switch_checked(edition, expected).await;
            print_json(
                &serde_json::json!({"requested_account":args[1],"report":result.report,"outcome":result.outcome}),
            )?;
            Ok(if result.outcome.completed { 0 } else { 2 })
        }
        Some("fix" | "set-region") => repair_cli::run(RepairCommand::parse(&args)?, edition).await,
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

#[tokio::main]
async fn main() {
    let code = match execute().await {
        Ok(code) => code,
        Err(error) => {
            let _ = print_json(
                &serde_json::json!({"schema_version":1,"error":error,"recovery":"按错误提示检查配置和命令；账号事务中断时保留资料并执行 recover"}),
            );
            2
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
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
