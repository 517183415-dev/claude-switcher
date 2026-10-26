//! Restricted command-line entry for the installed GUI's repair identity.
use std::{
    io::Write,
    path::{Path, PathBuf},
};
#[cfg(target_os = "macos")]
use switcher_domain::system::{ClientRole, InstallRecord, SOURCE_LOCAL_TRANSACTION};
use switcher_domain::{Edition, desktop::RegionPreset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairCommand {
    Fix { apply: bool },
    DetectRegion { apply: bool },
    SetRegion { preset: RegionPreset, apply: bool },
}

impl RepairCommand {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        match args {
            [command] if command == "fix" => Ok(Self::Fix { apply: true }),
            [command, flag] if command == "fix" && flag == "--preview" => {
                Ok(Self::Fix { apply: false })
            }
            [command, flag] if command == "fix" && flag == "--apply" => {
                Ok(Self::Fix { apply: true })
            }
            [command] if command == "set-region" => Ok(Self::DetectRegion { apply: true }),
            [command, flag] if command == "set-region" && flag == "--preview" => {
                Ok(Self::DetectRegion { apply: false })
            }
            [command, flag] if command == "set-region" && flag == "--apply" => {
                Ok(Self::DetectRegion { apply: true })
            }
            [command, name] | [command, name, _]
                if command == "set-region"
                    && (args.len() == 2 || matches!(args[2].as_str(), "--apply" | "--preview")) =>
            {
                let preset = serde_json::from_value(serde_json::Value::String(name.clone()))
                    .map_err(|_| "地区只接受已列出的固定国家枚举")?;
                Ok(Self::SetRegion {
                    preset,
                    apply: args.len() == 2 || args[2] == "--apply",
                })
            }
            _ => Err("invalid_command".into()),
        }
    }

    pub fn args(self) -> Vec<String> {
        match self {
            Self::Fix { apply } => with_apply(vec!["fix".into()], apply),
            Self::DetectRegion { apply } => with_apply(vec!["set-region".into()], apply),
            Self::SetRegion { preset, apply } => with_apply(
                vec![
                    "set-region".into(),
                    serde_json::to_value(preset)
                        .expect("fixed region enum")
                        .as_str()
                        .expect("region string")
                        .into(),
                ],
                apply,
            ),
        }
    }
}

fn with_apply(mut args: Vec<String>, apply: bool) -> Vec<String> {
    if !apply {
        args.push("--preview".into());
    }
    args
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputMode {
    Modern,
    Legacy,
}

trait RepairBackend {
    type Prepared;
    async fn prepare(
        &mut self,
        command: RepairCommand,
        edition: Edition,
    ) -> Result<Self::Prepared, String>;
    fn preview(prepared: &Self::Prepared) -> &switcher_domain::desktop::RepairPreview;
    fn apply(&mut self, prepared: Self::Prepared) -> Result<(), String>;
    async fn check(&mut self) -> Result<switcher_domain::checks::CheckReport, String>;
}

struct NativeRepair;
impl RepairBackend for NativeRepair {
    type Prepared = (
        crate::regions::RegionCoordinator,
        crate::regions::PreparedRepair,
    );
    async fn prepare(
        &mut self,
        command: RepairCommand,
        edition: Edition,
    ) -> Result<Self::Prepared, String> {
        let coordinator = crate::regions::RegionCoordinator::for_current_user(edition)?;
        let prepared = match command {
            RepairCommand::Fix { .. } => coordinator.preview(None)?,
            RepairCommand::SetRegion { preset, .. } => coordinator.preview(Some(preset))?,
            RepairCommand::DetectRegion { .. } => {
                let client = crate::network::inherited_client()?;
                let detected = crate::region_detection::detect(&client).await?;
                coordinator.preview_detected(detected)?
            }
        };
        Ok((coordinator, prepared))
    }
    fn preview(prepared: &Self::Prepared) -> &switcher_domain::desktop::RepairPreview {
        &prepared.1.preview
    }
    async fn check(&mut self) -> Result<switcher_domain::checks::CheckReport, String> {
        let home = crate::config::home_dir()?;
        let configuration = crate::config::load_read_only(&home)?;
        Ok(crate::checks::run_checks(configuration).await)
    }
    fn apply(&mut self, (coordinator, prepared): Self::Prepared) -> Result<(), String> {
        coordinator.execute(prepared).map_err(str::to_owned)
    }
}

pub async fn run(command: RepairCommand, edition: Edition) -> Result<i32, String> {
    run_with_output(command, edition, OutputMode::Modern, &mut std::io::stdout()).await
}

pub async fn run_with_output(
    command: RepairCommand,
    edition: Edition,
    mode: OutputMode,
    output: &mut impl Write,
) -> Result<i32, String> {
    run_with_backend(command, edition, mode, &mut NativeRepair, output).await
}

async fn run_with_backend<B: RepairBackend>(
    command: RepairCommand,
    edition: Edition,
    mode: OutputMode,
    backend: &mut B,
    output: &mut impl Write,
) -> Result<i32, String> {
    // set-region retains its modern protocol, including when reached via the legacy shim.
    let legacy = mode == OutputMode::Legacy && matches!(command, RepairCommand::Fix { .. });
    let result: Result<i32, String> = async {
        if legacy { writeln!(output, "FIX:prepare").map_err(|_| "output_failed")?; }
        let prepared = backend.prepare(command, edition).await?;
        if legacy { writeln!(output, "FIX:preview").map_err(|_| "output_failed")?; }
        else { write_json(output, B::preview(&prepared))?; }
        if matches!(command, RepairCommand::Fix { apply: true } | RepairCommand::SetRegion { apply: true, .. } | RepairCommand::DetectRegion { apply: true }) {
            if legacy { writeln!(output, "FIX:apply").map_err(|_| "output_failed")?; }
            backend.apply(prepared)?;
            if matches!(command, RepairCommand::Fix { apply: true }) {
                if legacy { writeln!(output, "FIX:check").map_err(|_| "output_failed")?; }
                let report = backend.check().await?;
                let allowed = report.launch.allowed;
                if legacy {
                    write_json(output, &report)?;
                    if !allowed { return Err("修复已执行，但完整环境检查未通过".into()); }
                } else {
                    write_json(output, &serde_json::json!({
                        "completed": allowed,
                        "message": if allowed { "修复完成，完整环境检查已通过" } else { "修复已执行，但完整环境检查未通过" },
                        "recovery": if allowed { None } else { Some("修复阻塞项后重新检查") },
                        "report": report
                    }))?;
                    return Ok(if allowed { 0 } else { 1 });
                }
            }
            if legacy { writeln!(output, "FIX:complete\nOK").map_err(|_| "output_failed")?; }
            else { write_json(output, &serde_json::json!({"completed":true,"message":"系统修复已完成并保存地区目标；请重新检查环境与地区格式","recovery":null}))?; }
        }
        Ok(0)
    }.await;
    match result {
        Err(error) if legacy => {
            writeln!(output, "FAIL:{}", error.replace(['\r', '\n'], " "))
                .map_err(|_| "output_failed")?;
            Ok(1)
        }
        result => result,
    }
}

fn write_json(output: &mut impl Write, value: &impl serde::Serialize) -> Result<(), &'static str> {
    let text = serde_json::to_string(value).map_err(|_| "serialization_failed")?;
    writeln!(output, "{text}").map_err(|_| "output_failed")
}

pub fn print_json(value: &impl serde::Serialize) -> Result<(), &'static str> {
    let output = serde_json::to_string(value).map_err(|_| "serialization_failed")?;
    println!("{output}");
    Ok(())
}

fn app(edition: Edition) -> PathBuf {
    PathBuf::from(match edition {
        Edition::Personal => "/Applications/Claude 切换器 Personal.app",
        Edition::Public => "/Applications/Claude 切换器 Public.app",
    })
}

/// Validate the installed identity before launching the fixed GUI binary.
/// The helper performs its own code-identity check on the new process.
#[cfg(target_os = "macos")]
pub fn installed_gui(edition: Edition) -> Result<PathBuf, &'static str> {
    let uid = unsafe { libc::geteuid() };
    let record = crate::install::validate_installed_identity(edition, uid)?;
    match std::fs::symlink_metadata(SOURCE_LOCAL_TRANSACTION) {
        Ok(_) => return Err("installation_replacement_pending"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("installation_replacement_unknown"),
    }
    let app = app(edition);
    let requirement = gui_requirement(edition, &record)?;
    verify_gui(&app, &requirement, |path, requirement| {
        std::process::Command::new("/usr/bin/codesign")
            .args([
                "--verify",
                "--strict",
                "--test-requirement",
                &format!("={requirement}"),
            ])
            .arg(path)
            .output()
            .map(|output| output.status.success())
            .map_err(|_| "installed_gui_signature_unavailable")
    })
}

#[cfg(target_os = "macos")]
fn gui_requirement(edition: Edition, record: &InstallRecord) -> Result<String, &'static str> {
    if record.edition != edition || record.owner_uid != unsafe { libc::geteuid() } {
        return Err("installed_owner_or_edition");
    }
    record.client_requirement(ClientRole::Gui)
}

#[cfg(target_os = "macos")]
fn verify_gui(
    app: &Path,
    requirement: &str,
    verify: impl FnOnce(&Path, &str) -> Result<bool, &'static str>,
) -> Result<PathBuf, &'static str> {
    if !verify(app, requirement)? {
        return Err("installed_gui_identity");
    }
    let executable = app.join("Contents/MacOS/switcher-desktop");
    if !executable.is_file() {
        return Err("installed_gui_executable");
    }
    Ok(executable)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct F06Repair {
        fail_prepare: bool,
        fail_apply: bool,
        check_status: Option<switcher_domain::checks::CheckStatus>,
        calls: Vec<&'static str>,
    }
    impl RepairBackend for F06Repair {
        type Prepared = switcher_domain::desktop::RepairPreview;
        async fn prepare(
            &mut self,
            _: RepairCommand,
            _: Edition,
        ) -> Result<Self::Prepared, String> {
            self.calls.push("prepare");
            if self.fail_prepare {
                return Err("synthetic_precondition".into());
            }
            Ok(switcher_domain::desktop::RepairPreview {
                can_execute: true,
                recovery_required: false,
                message: "synthetic preview".into(),
                original: crate::regions::preset(RegionPreset::Us),
                target: crate::regions::preset(RegionPreset::Jp),
                network_service: "synthetic".into(),
                original_ipv6: "auto".into(),
                target_ipv6: "off".into(),
                user_changes: vec![],
            })
        }
        async fn check(&mut self) -> Result<switcher_domain::checks::CheckReport, String> {
            use switcher_domain::checks::*;
            self.calls.push("check");
            let status = self.check_status.ok_or("synthetic_check_error")?;
            let checks: Vec<_> = CheckId::ALL
                .into_iter()
                .map(|id| CheckResult {
                    id,
                    status,
                    reason: "synthetic".into(),
                    observed_at: 100,
                    duration_ms: 1,
                    valid_until: 130,
                    network_generation: "synthetic".into(),
                    evidence: vec![],
                })
                .collect();
            let summary = crate::checks::summarize(&checks);
            let mut report = CheckReport {
                schema_version: 1,
                observed_at: 100,
                completed_at: 101,
                duration_ms: 1,
                network_generation: "synthetic".into(),
                checks,
                summary,
                launch: LaunchDecision {
                    allowed: false,
                    reasons: vec![],
                },
            };
            report.launch = crate::checks::evaluate_launch(&report, 101, "synthetic");
            Ok(report)
        }
        fn preview(prepared: &Self::Prepared) -> &switcher_domain::desktop::RepairPreview {
            prepared
        }
        fn apply(&mut self, _: Self::Prepared) -> Result<(), String> {
            self.calls.push("apply");
            if self.fail_apply {
                Err("synthetic_partial_failure".into())
            } else {
                Ok(())
            }
        }
    }
    async fn f06_repair_case(
        name: &str,
        command: RepairCommand,
        mode: OutputMode,
        fail_prepare: bool,
        fail_apply: bool,
    ) -> (Result<i32, String>, String, Vec<&'static str>) {
        let mut backend = F06Repair {
            fail_prepare,
            fail_apply,
            check_status: Some(switcher_domain::checks::CheckStatus::Pass),
            calls: vec![],
        };
        let mut output = Vec::new();
        let result =
            run_with_backend(command, Edition::Personal, mode, &mut backend, &mut output).await;
        let stdout = String::from_utf8(output).unwrap();
        println!(
            "F06_REPAIR_RECEIPT {}",
            serde_json::json!({"case":name,"stdout":stdout,"stderr":"","code": result.as_ref().copied().unwrap_or(2),"result":result,"calls":backend.calls,"preview_calls":backend.calls.iter().filter(|call| **call == "prepare").count(),"execute_calls":backend.calls.iter().filter(|call| **call == "apply").count()})
        );
        (result, stdout, backend.calls)
    }
    #[tokio::test]
    async fn f06_legacy_repair_preview_only_has_no_apply_or_ok() {
        let (result, output, calls) = f06_repair_case(
            "legacy-preview",
            RepairCommand::Fix { apply: false },
            OutputMode::Legacy,
            false,
            false,
        )
        .await;
        assert_eq!(result, Ok(0));
        assert_eq!(output, "FIX:prepare\nFIX:preview\n");
        assert_eq!(calls, ["prepare"]);
    }
    #[tokio::test]
    async fn f06_legacy_repair_complete_has_real_events_and_ok() {
        let (result, output, calls) = f06_repair_case(
            "legacy-complete",
            RepairCommand::Fix { apply: true },
            OutputMode::Legacy,
            false,
            false,
        )
        .await;
        assert_eq!(result, Ok(0));
        assert!(output.starts_with("FIX:prepare\nFIX:preview\nFIX:apply\nFIX:check\n"));
        assert!(output.ends_with("FIX:complete\nOK\n"));
        let report: switcher_domain::checks::CheckReport =
            serde_json::from_str(output.lines().nth(4).unwrap()).unwrap();
        assert_eq!(report.checks.len(), 14);
        assert!(report.launch.allowed);
        assert_eq!(calls, ["prepare", "apply", "check"]);
    }
    #[tokio::test]
    async fn f06_legacy_repair_prepare_failure_has_no_apply_or_ok() {
        let (result, output, calls) = f06_repair_case(
            "legacy-prepare-error",
            RepairCommand::Fix { apply: true },
            OutputMode::Legacy,
            true,
            false,
        )
        .await;
        assert_eq!(result, Ok(1));
        assert_eq!(output, "FIX:prepare\nFAIL:synthetic_precondition\n");
        assert_eq!(calls, ["prepare"]);
    }
    #[tokio::test]
    async fn f06_legacy_repair_partial_failure_has_no_complete_or_ok() {
        let (result, output, calls) = f06_repair_case(
            "legacy-partial-error",
            RepairCommand::Fix { apply: true },
            OutputMode::Legacy,
            false,
            true,
        )
        .await;
        assert_eq!(result, Ok(1));
        assert_eq!(
            output,
            "FIX:prepare\nFIX:preview\nFIX:apply\nFAIL:synthetic_partial_failure\n"
        );
        assert_eq!(calls, ["prepare", "apply"]);
    }
    #[tokio::test]
    async fn f06_modern_repair_and_set_region_keep_json_and_errors() {
        for command in [
            RepairCommand::Fix { apply: true },
            RepairCommand::SetRegion {
                preset: RegionPreset::Us,
                apply: true,
            },
            RepairCommand::DetectRegion { apply: true },
        ] {
            for mode in [OutputMode::Modern, OutputMode::Legacy] {
                if mode == OutputMode::Legacy && matches!(command, RepairCommand::Fix { .. }) {
                    continue;
                }
                let (result, output, calls) = f06_repair_case(
                    &format!("modern-{command:?}-{mode:?}"),
                    command,
                    mode,
                    false,
                    false,
                )
                .await;
                assert_eq!(result, Ok(0));
                let fix = matches!(command, RepairCommand::Fix { .. });
                assert_eq!(
                    calls,
                    if fix {
                        vec!["prepare", "apply", "check"]
                    } else {
                        vec!["prepare", "apply"]
                    }
                );
                let rows: Vec<serde_json::Value> = output
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect();
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0]["message"], "synthetic preview");
                assert_eq!(rows[1]["completed"], true);
            }
        }
        let (result, output, calls) = f06_repair_case(
            "modern-partial-error",
            RepairCommand::Fix { apply: true },
            OutputMode::Modern,
            false,
            true,
        )
        .await;
        assert_eq!(result, Err("synthetic_partial_failure".into()));
        assert_eq!(output.lines().count(), 1);
        assert_eq!(calls, ["prepare", "apply"]);
        assert!(!output.contains("completed"));
    }

    #[tokio::test]
    async fn ordinary_fix_preview_and_prepare_or_apply_errors_never_recheck() {
        for mode in [OutputMode::Legacy, OutputMode::Modern] {
            for (apply, fail_prepare, fail_apply) in [
                (false, false, false),
                (true, true, false),
                (true, false, true),
            ] {
                let (_, output, calls) = f06_repair_case(
                    "no-recheck",
                    RepairCommand::Fix { apply },
                    mode,
                    fail_prepare,
                    fail_apply,
                )
                .await;
                assert!(!calls.contains(&"check"));
                assert!(!output.lines().any(|line| line == "OK"));
            }
        }
    }

    #[tokio::test]
    async fn ordinary_fix_checks_once_and_never_reports_success_for_blocked_or_failed_checks() {
        use switcher_domain::checks::{CheckId, CheckReport, CheckStatus};
        for mode in [OutputMode::Legacy, OutputMode::Modern] {
            for status in [
                Some(CheckStatus::Pass),
                Some(CheckStatus::Fail),
                Some(CheckStatus::Unknown),
                Some(CheckStatus::Timeout),
                None,
            ] {
                let mut backend = F06Repair {
                    fail_prepare: false,
                    fail_apply: false,
                    check_status: status,
                    calls: vec![],
                };
                let mut output = vec![];
                let result = run_with_backend(
                    RepairCommand::Fix { apply: true },
                    Edition::Personal,
                    mode,
                    &mut backend,
                    &mut output,
                )
                .await;
                let output = String::from_utf8(output).unwrap();
                assert_eq!(backend.calls, ["prepare", "apply", "check"]);
                let allowed = status == Some(CheckStatus::Pass);
                if status.is_none() {
                    assert_eq!(
                        result,
                        if mode == OutputMode::Legacy {
                            Ok(1)
                        } else {
                            Err("synthetic_check_error".into())
                        }
                    );
                } else {
                    assert_eq!(result, Ok(if allowed { 0 } else { 1 }));
                    let report: CheckReport = if mode == OutputMode::Legacy {
                        serde_json::from_str(output.lines().nth(4).unwrap()).unwrap()
                    } else {
                        let final_row: serde_json::Value =
                            serde_json::from_str(output.lines().last().unwrap()).unwrap();
                        assert_eq!(final_row["completed"], allowed);
                        serde_json::from_value(final_row["report"].clone()).unwrap()
                    };
                    assert_eq!(
                        report.checks.iter().map(|row| row.id).collect::<Vec<_>>(),
                        CheckId::ALL.to_vec()
                    );
                    assert_eq!(report.launch.allowed, allowed);
                }
                if !allowed {
                    assert!(
                        !output
                            .lines()
                            .any(|line| line == "OK" || line == "FIX:complete")
                    );
                    assert!(!output.contains("\"completed\":true"));
                }
            }
        }
    }

    fn parse(args: &[&str]) -> Result<RepairCommand, String> {
        RepairCommand::parse(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>())
    }

    #[test]
    fn fixed_commands_execute_by_default_with_explicit_preview() {
        assert_eq!(parse(&["fix"]).unwrap(), RepairCommand::Fix { apply: true });
        assert_eq!(
            parse(&["fix", "--preview"]).unwrap(),
            RepairCommand::Fix { apply: false }
        );
        assert_eq!(
            parse(&["fix", "--apply"]).unwrap(),
            RepairCommand::Fix { apply: true }
        );
        assert_eq!(
            parse(&["set-region"]).unwrap(),
            RepairCommand::DetectRegion { apply: true }
        );
        assert_eq!(
            parse(&["set-region", "--preview"]).unwrap(),
            RepairCommand::DetectRegion { apply: false }
        );
        assert!(matches!(
            parse(&["set-region", "us"]),
            Ok(RepairCommand::SetRegion { apply: true, .. })
        ));
        assert!(matches!(
            parse(&["set-region", "jp", "--preview"]),
            Ok(RepairCommand::SetRegion { apply: false, .. })
        ));
    }

    #[test]
    fn command_allowlist_rejects_extra_arguments_and_paths() {
        for args in [
            vec!["fix", "--apply", "other"],
            vec!["set-region", "us", "--apply", "other"],
            vec!["set-region", "bad"],
            vec!["switch", "A"],
            vec!["fix", "/bin/sh"],
        ] {
            assert!(parse(&args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    fn serialized_forwarding_keeps_only_fixed_arguments() {
        for args in [
            vec!["fix"],
            vec!["fix", "--preview"],
            vec!["set-region"],
            vec!["set-region", "us", "--preview"],
        ] {
            assert_eq!(parse(&args).unwrap().args(), args);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn edition_mismatch_and_unverified_gui_are_rejected() {
        use switcher_domain::system::InstallTrust;
        let record = InstallRecord {
            schema_version: 2,
            owner_uid: unsafe { libc::geteuid() },
            edition: Edition::Personal,
            trust: InstallTrust::SourceLocal {
                generation: "a".repeat(32),
                gui_cdhash: "1".repeat(40),
                guardian_cdhash: "2".repeat(40),
                helper_cdhash: "3".repeat(40),
            },
            helper_version: "0.2.0".into(),
            minimum_app_version: "0.2.0".into(),
        };
        assert_eq!(
            gui_requirement(Edition::Public, &record),
            Err("installed_owner_or_edition")
        );
        let requirement = gui_requirement(Edition::Personal, &record).unwrap();
        assert!(requirement.contains("cdhash H\"1111"));
        assert_eq!(
            verify_gui(Path::new("/fixed/app"), &requirement, |path, actual| {
                assert_eq!(path, Path::new("/fixed/app"));
                assert_eq!(actual, requirement);
                Ok(false)
            }),
            Err("installed_gui_identity")
        );
    }
}
