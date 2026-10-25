//! Repair only the installed, pinned user Guardian registration.
use std::{
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use switcher_domain::{
    Edition,
    system::{ClientRole, GUARDIAN_LABEL, InstallRecord},
};

const PLIST: &str = "/Library/LaunchAgents/local.claudeswitcher.guardian.plist";
const BINARY: &str = "/Library/Application Support/claude-switcher/switcher-guardian";
const TEMPLATE: &[u8] =
    include_bytes!("../../../packaging/macos/launchd/local.claudeswitcher.guardian.plist");
const REINSTALL: &str = "守护组件缺失或校验失败；请重新运行安装器";
const INCOMPLETE: &str =
    "系统和用户配置已提交，但守护服务尚未恢复；请重新预览修复，当前不能启动 Claude";

fn safe_file(path: &Path, contents: Option<&[u8]>) -> Result<(), &'static str> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    for parent in path.ancestors().skip(1) {
        let m = std::fs::symlink_metadata(parent).map_err(|_| REINSTALL)?;
        if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Err(REINSTALL);
        }
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| REINSTALL)?;
    let m = file.metadata().map_err(|_| REINSTALL)?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o022 != 0 || m.nlink() != 1 {
        return Err(REINSTALL);
    }
    if let Some(expected) = contents {
        let mut bytes = Vec::new();
        file.take(expected.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| REINSTALL)?;
        validate_plist(&bytes)?;
    }
    Ok(())
}
fn validate_plist(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes == TEMPLATE {
        Ok(())
    } else {
        Err(REINSTALL)
    }
}
fn command(program: &str, args: &[&str]) -> Result<bool, &'static str> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| INCOMPLETE)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(INCOMPLETE);
            }
        }
    }
}
pub(crate) fn validate(edition: Edition) -> Result<InstallRecord, &'static str> {
    let record = crate::install::validate_installed_identity(edition, unsafe { libc::geteuid() })?;
    safe_file(Path::new(PLIST), Some(TEMPLATE))?;
    safe_file(Path::new(BINARY), None)?;
    let requirement = format!("={}", record.client_requirement(ClientRole::Guardian)?);
    if !command(
        "/usr/bin/codesign",
        &[
            "--verify",
            "--strict",
            "--test-requirement",
            &requirement,
            BINARY,
        ],
    )
    .map_err(|_| REINSTALL)?
    {
        return Err(REINSTALL);
    }
    Ok(record)
}

fn bound_health<R: PartialEq>(
    expected: &R,
    mut current: impl FnMut() -> Result<R, &'static str>,
    healthy: impl FnOnce() -> bool,
) -> Result<bool, &'static str> {
    if current()? != *expected {
        return Err(INCOMPLETE);
    }
    let result = healthy();
    if current()? != *expected {
        return Err(INCOMPLETE);
    }
    Ok(result)
}

fn repair_with<T>(
    validate: impl FnOnce() -> Result<T, &'static str>,
    mut healthy: impl FnMut() -> bool,
    registered: impl FnOnce() -> Result<bool, &'static str>,
    restart: impl FnOnce(bool) -> Result<(), &'static str>,
    mut wait: impl FnMut() -> bool,
) -> Result<(), &'static str> {
    let guard = validate().map_err(|_| INCOMPLETE)?;
    if healthy() {
        return Ok(());
    }
    restart(registered().map_err(|_| INCOMPLETE)?).map_err(|_| INCOMPLETE)?;
    drop(guard); // Guardian needs the lifecycle lock to publish current health.
    loop {
        if healthy() {
            return Ok(());
        }
        if !wait() {
            return Err(INCOMPLETE);
        }
    }
}
/// Caller has committed and released the lifecycle lock before waiting for health.
pub(crate) fn restore_after_commit(edition: Edition) -> Result<(), &'static str> {
    let uid = unsafe { libc::geteuid() };
    let domain = format!("gui/{uid}");
    let target = format!("{domain}/{GUARDIAN_LABEL}");
    let deadline = Instant::now() + Duration::from_secs(10);
    let expected = std::cell::RefCell::new(None);
    let identity_changed = std::cell::Cell::new(false);
    repair_with(
        || {
            let home = crate::migration::account_home(uid).map_err(|_| INCOMPLETE)?;
            let lock =
                crate::storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2"))
                    .map_err(|_| INCOMPLETE)?;
            *expected.borrow_mut() = Some(validate(edition)?);
            Ok(lock)
        },
        || {
            if identity_changed.get() {
                return false;
            }
            expected.borrow().as_ref().is_some_and(|record| {
                bound_health(record, crate::guardian::read_install_record, || {
                    crate::install::validate_installed_health(uid).is_ok()
                })
                .unwrap_or_else(|_| {
                    identity_changed.set(true);
                    false
                })
            })
        },
        || {
            if identity_changed.get() {
                Err(INCOMPLETE)
            } else {
                command("/bin/launchctl", &["print", &target])
            }
        },
        |registered| {
            let ok = if registered {
                command("/bin/launchctl", &["kickstart", "-k", &target])?
            } else {
                command("/bin/launchctl", &["bootstrap", &domain, PLIST])?
            };
            if ok { Ok(()) } else { Err(INCOMPLETE) }
        },
        || {
            if identity_changed.get() || Instant::now() >= deadline {
                false
            } else {
                std::thread::sleep(Duration::from_millis(100));
                true
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guardian_repair_rejects_installation_change_before_or_during_health() {
        assert!(bound_health(&1, || Ok(2), || panic!("different installation")).is_err());
        let mut reads = 0;
        assert!(
            bound_health(
                &1,
                || {
                    reads += 1;
                    Ok(reads)
                },
                || true
            )
            .is_err()
        );
        assert_eq!(bound_health(&1, || Ok(1), || true), Ok(true));
    }
    #[test]
    fn guardian_repair_wait_does_not_accept_health_from_replaced_installation() {
        let generation = std::cell::Cell::new(1);
        let running = std::cell::Cell::new(false);
        assert_eq!(
            repair_with(
                || Ok(()),
                || bound_health(&1, || Ok(generation.get()), || running.get()).unwrap_or(false),
                || Ok(false),
                |_| {
                    generation.set(2);
                    running.set(true);
                    Ok(())
                },
                || false,
            ),
            Err(INCOMPLETE)
        );
    }
    #[test]
    fn guardian_repair_rejects_modified_plist() {
        assert!(validate_plist(TEMPLATE).is_ok());
        assert_eq!(
            validate_plist(b"malicious or incomplete plist"),
            Err(REINSTALL)
        );
    }
    #[test]
    fn guardian_repair_rejects_user_writable_component_paths() {
        let path = std::env::temp_dir().join(format!("guardian-unsafe-{}", std::process::id()));
        std::fs::write(&path, TEMPLATE).unwrap();
        assert_eq!(safe_file(&path, Some(TEMPLATE)), Err(REINSTALL));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn guardian_repair_identity_failure_and_healthy_noop_never_launch() {
        assert_eq!(
            repair_with(
                || Err::<(), _>("identity"),
                || panic!(),
                || panic!(),
                |_| panic!(),
                || panic!()
            ),
            Err(INCOMPLETE)
        );
        assert!(repair_with(|| Ok(()), || true, || panic!(), |_| panic!(), || panic!()).is_ok());
    }
    #[test]
    fn guardian_repair_missing_and_stale_heartbeat_use_correct_action() {
        for registered in [false, true] {
            let mut checks = 0;
            assert!(
                repair_with(
                    || Ok(()),
                    || {
                        checks += 1;
                        checks > 2
                    },
                    || Ok(registered),
                    |action| {
                        assert_eq!(action, registered);
                        Ok(())
                    },
                    || true
                )
                .is_ok()
            );
        }
    }
    #[test]
    fn guardian_repair_releases_capability_before_waiting_for_health() {
        use std::cell::Cell;
        struct Guard<'a>(&'a Cell<bool>);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let locked = Cell::new(false);
        let calls = Cell::new(0);
        assert!(
            repair_with(
                || {
                    locked.set(true);
                    Ok(Guard(&locked))
                },
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        false
                    } else {
                        assert!(!locked.get());
                        true
                    }
                },
                || Ok(false),
                |_| {
                    assert!(locked.get());
                    Ok(())
                },
                || panic!("second health observation should finish"),
            )
            .is_ok()
        );
    }
    #[test]
    fn guardian_repair_failure_reports_committed_configuration() {
        assert_eq!(
            repair_with(
                || Ok(()),
                || false,
                || Ok(false),
                |_| Err("bootstrap"),
                || panic!()
            ),
            Err(INCOMPLETE)
        );
        assert_eq!(
            repair_with(|| Ok(()), || false, || Ok(true), |_| Ok(()), || false),
            Err(INCOMPLETE)
        );
        assert!(INCOMPLETE.contains("已提交"));
    }
}

fn validate_root_plist(bytes: &[u8], template: &[u8]) -> Result<(), &'static str> {
    let (label, script, interval) =
        if template == include_bytes!("../../../daemons/local.claude-bodyguard.plist") {
            ("local.claude-bodyguard", "claude-bodyguard.sh", 10)
        } else if template == include_bytes!("../../../daemons/local.tz-watchdog.plist") {
            ("local.tz-watchdog", "tz-watchdog.sh", 3600)
        } else {
            return Err(REINSTALL);
        };
    // Two complete reviewed encodings, not a permissive XML normalization.
    // Cross-language tests feed the current Python producer into this comparison.
    let canonical = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\t<key>Label</key>\n\t<string>{label}</string>\n\
\t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>/bin/bash</string>\n\
\t\t<string>/Library/Application Support/claude-switcher/{script}</string>\n\t</array>\n\
\t<key>RunAtLoad</key>\n\t<true/>\n\t<key>StartInterval</key>\n\t<integer>{interval}</integer>\n\
</dict>\n</plist>\n"
    );
    if bytes == template || bytes == canonical.as_bytes() {
        Ok(())
    } else {
        Err(REINSTALL)
    }
}

fn root_parent_metadata(directory: bool, uid: u32, mode: u32) -> Result<(), &'static str> {
    if directory && uid == 0 && mode & 0o022 == 0 {
        Ok(())
    } else {
        Err(REINSTALL)
    }
}

fn root_file_metadata(
    metadata: &std::fs::Metadata,
    uid: u32,
    gid: u32,
    mode: u32,
) -> Result<(), &'static str> {
    use std::os::unix::fs::MetadataExt;
    if metadata.is_file()
        && uid == 0
        && gid == 0
        && metadata.mode() & 0o7777 == mode
        && metadata.nlink() == 1
    {
        Ok(())
    } else {
        Err(REINSTALL)
    }
}

fn read_checked_root_file(
    path: &Path,
    limit: u64,
    check: impl FnOnce(&std::fs::Metadata) -> Result<(), &'static str>,
) -> Result<Vec<u8>, &'static str> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| REINSTALL)?;
    check(&file.metadata().map_err(|_| REINSTALL)?)?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| REINSTALL)?;
    if bytes.len() as u64 > limit {
        return Err(REINSTALL);
    }
    Ok(bytes)
}

fn read_root_file(path: &Path, mode: u32, limit: u64) -> Result<Vec<u8>, &'static str> {
    use std::os::unix::fs::MetadataExt;
    for parent in path.ancestors().skip(1) {
        let m = std::fs::symlink_metadata(parent).map_err(|_| REINSTALL)?;
        root_parent_metadata(m.is_dir(), m.uid(), m.mode())?;
    }
    read_checked_root_file(path, limit, |m| {
        root_file_metadata(m, m.uid(), m.gid(), mode)
    })
}

/// Restore only existing root-owned daemon plists and fixed protected executables.
/// Missing artifacts are reported; no user-writable program is copied or elevated.
pub(crate) fn legacy_admin_commands() -> (Vec<String>, bool) {
    legacy_admin_commands_with(
        |label| command("/bin/launchctl", &["print", &format!("system/{label}")]),
        |plist, executable, template| {
            match std::fs::symlink_metadata(plist) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(_) => return Err(REINSTALL),
                Ok(_) => {}
            }
            validate_root_plist(&read_root_file(Path::new(plist), 0o644, 4096)?, template)?;
            read_root_file(Path::new(executable), 0o755, 64 * 1024)?;
            Ok(true)
        },
        || {
            if !Path::new(PLIST).exists() {
                return Ok(false);
            }
            safe_file(Path::new(PLIST), Some(TEMPLATE))?;
            safe_file(Path::new(BINARY), None)?;
            let domain = format!("gui/{}", unsafe { libc::geteuid() });
            let target = format!("{domain}/{GUARDIAN_LABEL}");
            if command("/bin/launchctl", &["print", &target])?
                || command("/bin/launchctl", &["bootstrap", &domain, PLIST])?
            {
                Ok(true)
            } else {
                Err(REINSTALL)
            }
        },
    )
}

fn legacy_admin_commands_with(
    mut registered: impl FnMut(&str) -> Result<bool, &'static str>,
    mut components: impl FnMut(&str, &str, &[u8]) -> Result<bool, &'static str>,
    user_guardian: impl FnOnce() -> Result<bool, &'static str>,
) -> (Vec<String>, bool) {
    let mut commands = Vec::new();
    let mut bodyguard_available = false;
    let mut failed = false;
    for (label, plist, executable, template) in [
        (
            "local.claude-bodyguard",
            "/Library/LaunchDaemons/local.claude-bodyguard.plist",
            "/Library/Application Support/claude-switcher/claude-bodyguard.sh",
            &include_bytes!("../../../daemons/local.claude-bodyguard.plist")[..],
        ),
        (
            "local.tz-watchdog",
            "/Library/LaunchDaemons/local.tz-watchdog.plist",
            "/Library/Application Support/claude-switcher/tz-watchdog.sh",
            &include_bytes!("../../../daemons/local.tz-watchdog.plist")[..],
        ),
    ] {
        let registration = registered(label);
        failed |= registration.is_err();
        let available = match components(plist, executable, template) {
            Ok(true) => {
                if !matches!(registration, Ok(true)) {
                    commands.push(format!("/bin/launchctl bootstrap system '{plist}'"));
                }
                true
            }
            Ok(false) => false,
            Err(_) => {
                failed = true;
                false
            }
        };
        if label == "local.claude-bodyguard" {
            bodyguard_available = available;
        }
    }
    let user_available = match user_guardian() {
        Ok(available) => available,
        Err(_) => {
            failed = true;
            false
        }
    };
    (commands, !failed && (bodyguard_available || user_available))
}

#[cfg(test)]
mod legacy_tests {
    use super::*;

    fn python_artifacts() -> serde_json::Value {
        let output = std::process::Command::new("python3")
            .args(["-B", "-c", r#"
import importlib.util, json, hashlib, sys
from pathlib import Path
path=Path(sys.argv[1])/'packaging/macos/local_install.py'
spec=importlib.util.spec_from_file_location('f03_install',path)
m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
values={'watchdog':m.watchdog_payload('Asia/Tokyo')[1][2].decode(),
        'bodyguard':m.bodyguard_payload('auto')[1][2].decode()}
print(json.dumps(dict(values=values,python=sys.executable,source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    artifact_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in values.items()})))
"#])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .output().expect("F03 requires the existing Python interpreter");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        println!("F03 Python artifact receipt: {value}");
        value
    }

    #[test]
    fn f03_actual_python_plists_and_bootstrap_contract() {
        let value = python_artifacts();
        for body_registered in [false, true] {
            for watchdog_registered in [false, true] {
                let mut checks = 0;
                let (commands, ready) = legacy_admin_commands_with(
                    |label| {
                        Ok(if label == "local.claude-bodyguard" {
                            body_registered
                        } else {
                            watchdog_registered
                        })
                    },
                    |plist, _, template| {
                        checks += 1;
                        let key = if plist.ends_with("local.tz-watchdog.plist") {
                            "watchdog"
                        } else {
                            "bodyguard"
                        };
                        validate_root_plist(
                            value["values"][key].as_str().unwrap().as_bytes(),
                            template,
                        )?;
                        Ok(true)
                    },
                    || Ok(false),
                );
                assert!(ready, "valid Python root services must be available");
                assert_eq!(checks, 2, "registered services must also validate");
                let mut expected = Vec::new();
                if !body_registered {
                    expected.push("/bin/launchctl bootstrap system '/Library/LaunchDaemons/local.claude-bodyguard.plist'");
                }
                if !watchdog_registered {
                    expected.push("/bin/launchctl bootstrap system '/Library/LaunchDaemons/local.tz-watchdog.plist'");
                }
                assert_eq!(commands, expected);
            }
        }
    }

    #[test]
    fn f03_registered_corrupt_components_do_not_become_ready() {
        let (commands, ready) =
            legacy_admin_commands_with(|_| Ok(true), |_, _, _| Err(REINSTALL), || Ok(false));
        assert!(!ready);
        assert!(commands.is_empty());
    }

    #[test]
    fn f03_approved_encodings_reject_execution_contract_changes() {
        let artifacts = python_artifacts();
        for (key, template, interval) in [
            (
                "bodyguard",
                &include_bytes!("../../../daemons/local.claude-bodyguard.plist")[..],
                "10",
            ),
            (
                "watchdog",
                &include_bytes!("../../../daemons/local.tz-watchdog.plist")[..],
                "3600",
            ),
        ] {
            assert_eq!(validate_root_plist(template, template), Ok(()));
            let source = artifacts["values"][key].as_str().unwrap();
            assert_eq!(validate_root_plist(source.as_bytes(), template), Ok(()));
            let mutations = [
                source.replace("</dict>", "<key>Program</key><string>/bin/evil</string></dict>"),
                source.replace("</dict>", "<key>EnvironmentVariables</key><dict><key>BASH_ENV</key><string>/evil</string></dict></dict>"),
                source.replace("</dict>", "<key>Label</key><string>duplicate</string></dict>"),
                source.replace("local.", "foreign."),
                source.replace("/bin/bash", "/bin/sh"),
                source.replace("/Library/Application Support/", "/tmp/"),
                source.replace("</array>", "<string>extra</string></array>"),
                source.replace("<true/>", "<false/>"),
                source.replace("<true/>", "<string>true</string>"),
                source.replace(&format!("<integer>{interval}</integer>"), "<integer>99</integer>"),
                source.replace(&format!("<integer>{interval}</integer>"), &format!("<string>{interval}</string>")),
                source.replace("\t\t<string>/bin/bash</string>\n", "").replace("\t</array>", "\t\t<string>/bin/bash</string>\n\t</array>"),
                format!("{source}\n"),
            ];
            for changed in mutations {
                assert_ne!(changed, source);
                assert_eq!(
                    validate_root_plist(changed.as_bytes(), template),
                    Err(REINSTALL)
                );
                for registered in [false, true] {
                    let (commands, ready) = legacy_admin_commands_with(
                        |_| Ok(registered),
                        |_, _, _| validate_root_plist(changed.as_bytes(), template).map(|()| true),
                        || Ok(false),
                    );
                    assert!(!ready);
                    assert!(commands.is_empty());
                }
            }
        }
        assert!(validate_root_plist(TEMPLATE, TEMPLATE).is_err());
        assert!(
            validate_plist(include_bytes!("../../../daemons/local.tz-watchdog.plist")).is_err()
        );
    }

    #[test]
    fn f03_checked_descriptor_metadata_links_and_bounds() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = std::env::temp_dir().join(format!(
            "f03-root-files-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("component");
        std::fs::write(&file, b"trusted").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let read = |path: &Path, limit| {
            read_checked_root_file(path, limit, |m| root_file_metadata(m, 0, 0, 0o644))
        };
        assert_eq!(read(&file, 7).unwrap(), b"trusted");
        assert!(read(&file, 6).is_err());
        let metadata = std::fs::metadata(&file).unwrap();
        assert!(root_file_metadata(&metadata, 501, 0, 0o644).is_err());
        assert!(root_file_metadata(&metadata, 0, 80, 0o644).is_err());
        assert!(root_file_metadata(&metadata, 0, 0, 0o755).is_err());
        for mode in [0o666, 0o664, 0o600, 0o4644] {
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
            assert!(read(&file, 7).is_err());
        }
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let link = directory.join("link");
        symlink(&file, &link).unwrap();
        assert!(read(&link, 7).is_err());
        std::fs::remove_file(&link).unwrap();
        symlink(directory.join("missing"), &link).unwrap();
        assert!(read(&link, 7).is_err());
        std::fs::remove_file(&link).unwrap();
        std::fs::hard_link(&file, &link).unwrap();
        assert!(read(&file, 7).is_err());
        std::fs::remove_file(&link).unwrap();
        assert!(read(&directory, 7).is_err());
        for (directory, uid, mode) in [
            (false, 0, 0o755),
            (true, 501, 0o755),
            (true, 0, 0o775),
            (true, 0, 0o777),
        ] {
            assert!(root_parent_metadata(directory, uid, mode).is_err());
        }
        assert_eq!(root_parent_metadata(true, 0, 0o755), Ok(()));
        // A pathname replacement after metadata validation cannot change the opened bytes.
        let bytes = read_checked_root_file(&file, 7, |m| {
            root_file_metadata(m, 0, 0, 0o644)?;
            std::fs::rename(&file, directory.join("original")).unwrap();
            std::fs::write(&file, b"foreign").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(bytes, b"trusted");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn f03_target_update_is_canonical_and_strict() {
        let command =
            plan_watchdog_update("#!/bin/bash\nTARGET=\"Asia/Tokyo\"\n", "America/New_York")
                .unwrap();
        assert!(command.contains("TARGET=\"America/New_York\""));
        for source in [
            "",
            "TARGET=\"Asia/Tokyo\"\nTARGET=Asia/Tokyo\n",
            "TARGET=\"Asia/Tokyo\"; evil\n",
            "TARGET=\"$(id)\"\n",
            "TARGET=Unknown/Invalid\n",
            "TARGET=../../etc/passwd\n",
            "TARGET=\"Asia/Tokyo\n",
        ] {
            assert!(
                plan_watchdog_update(source, "America/New_York").is_none(),
                "{source:?}"
            );
        }
        assert!(plan_watchdog_update("TARGET=Asia/Tokyo\n", "Asia/Tokyo").is_none());
    }

    #[test]
    fn f03_emit_watchdog_update() {
        let input = std::env::var_os("SWITCHER_F03_BRIDGE_INPUT");
        let output = std::env::var_os("SWITCHER_F03_BRIDGE_OUTPUT");
        match (input, output) {
            (Some(input), Some(output)) => {
                let value: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
                let command = plan_watchdog_update(
                    value["script"].as_str().unwrap(),
                    value["timezone"].as_str().unwrap(),
                );
                std::fs::write(
                    output,
                    serde_json::to_vec(&serde_json::json!({"command":command})).unwrap(),
                )
                .unwrap();
            }
            (None, None) => {
                assert!(plan_watchdog_update("TARGET=Asia/Tokyo\n", "Asia/Tokyo").is_none())
            }
            _ => panic!("both F03 bridge paths are required"),
        }
    }

    #[test]
    fn only_timezone_watchdog_never_satisfies_bodyguard_readiness() {
        for registered_watchdog in [true, false] {
            let (commands, ready) = legacy_admin_commands_with(
                |label| Ok(registered_watchdog && label == "local.tz-watchdog"),
                |plist, _, _| Ok(plist.ends_with("local.tz-watchdog.plist")),
                || Ok(false),
            );
            assert!(!ready);
            assert_eq!(commands.len(), usize::from(!registered_watchdog));
            if !registered_watchdog {
                assert!(commands[0].contains("local.tz-watchdog.plist"));
            }
        }
    }
    #[test]
    fn bodyguard_or_user_guardian_satisfies_readiness_without_watchdog_substitution() {
        for user in [false, true] {
            let (_, ready) = legacy_admin_commands_with(
                |label| Ok(!user && label == "local.claude-bodyguard"),
                |plist, _, _| Ok(!user && plist.ends_with("local.claude-bodyguard.plist")),
                || Ok(user),
            );
            assert!(ready);
        }
    }
    #[test]
    fn failed_bodyguard_does_not_discard_other_fixed_repairs() {
        let (commands, ready) = legacy_admin_commands_with(
            |_| Ok(false),
            |plist, _, _| {
                if plist.ends_with("local.claude-bodyguard.plist") {
                    Err(REINSTALL)
                } else {
                    Ok(true)
                }
            },
            || Ok(false),
        );
        assert!(!ready);
        assert_eq!(
            commands,
            ["/bin/launchctl bootstrap system '/Library/LaunchDaemons/local.tz-watchdog.plist'"]
        );
    }
}

pub(crate) fn legacy_watchdog_command(timezone: &str) -> Option<String> {
    if !crate::config::valid_timezone(timezone) {
        return None;
    }
    let path = Path::new("/Library/Application Support/claude-switcher/tz-watchdog.sh");
    let source = String::from_utf8(read_root_file(path, 0o755, 64 * 1024).ok()?).ok()?;
    plan_watchdog_update(&source, timezone)
}

fn plan_watchdog_update(source: &str, timezone: &str) -> Option<String> {
    if !crate::config::valid_timezone(timezone) {
        return None;
    }
    let mut assignments = source.split('\n').filter(|line| {
        let candidate = line.trim_start();
        candidate.starts_with("TARGET=") || candidate.starts_with("export TARGET=")
    });
    let value = assignments.next()?.strip_prefix("TARGET=")?;
    if assignments.next().is_some() {
        return None;
    }
    let target = if value.starts_with('"') {
        value.strip_prefix('"')?.strip_suffix('"')?
    } else {
        value
    };
    if !crate::config::valid_timezone(target) || target == timezone {
        return None;
    }
    Some(format!(
        "/usr/bin/sed -i '' 's|^TARGET=.*|TARGET=\"{timezone}\"|' '/Library/Application Support/claude-switcher/tz-watchdog.sh'"
    ))
}
