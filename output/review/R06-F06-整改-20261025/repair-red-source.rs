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
    _mode: OutputMode,
    backend: &mut B,
    output: &mut impl Write,
) -> Result<i32, String> {
    let prepared = backend.prepare(command, edition).await?;
    write_json(output, B::preview(&prepared))?;
    if matches!(
        command,
        RepairCommand::Fix { apply: true }
            | RepairCommand::SetRegion { apply: true, .. }
            | RepairCommand::DetectRegion { apply: true }
    ) {
        backend.apply(prepared)?;
        write_json(
            output,
            &serde_json::json!({"completed":true,"message":"系统修复已完成并保存地区目标；请重新检查环境与地区格式","recovery":null}),
        )?;
    }
    Ok(0)
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
            calls: vec![],
        };
        let mut output = Vec::new();
        let result =
            run_with_backend(command, Edition::Personal, mode, &mut backend, &mut output).await;
        let stdout = String::from_utf8(output).unwrap();
        println!(
            "F06_REPAIR_RECEIPT {}",
            serde_json::json!({"case":name,"stdout":stdout,"stderr":"","code": result.as_ref().copied().unwrap_or(2),"result":result,"calls":backend.calls})
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
        assert_eq!(
            output,
            "FIX:prepare\nFIX:preview\nFIX:apply\nFIX:complete\nOK\n"
        );
        assert_eq!(calls, ["prepare", "apply"]);
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
                assert_eq!(calls, ["prepare", "apply"]);
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
