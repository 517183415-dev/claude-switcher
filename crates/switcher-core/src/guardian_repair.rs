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

/// Restore only existing root-owned daemon plists and fixed protected executables.
/// Missing artifacts are reported; no user-writable program is copied or elevated.
pub(crate) fn legacy_admin_commands() -> (Vec<String>, bool) {
    legacy_admin_commands_with(
        |label| command("/bin/launchctl", &["print", &format!("system/{label}")]),
        |plist, executable, template| {
            if !Path::new(plist).exists() {
                return Ok(false);
            }
            safe_file(Path::new(plist), None)?;
            if std::fs::read(plist).map_err(|_| REINSTALL)? != template {
                return Err(REINSTALL);
            }
            safe_file(Path::new(executable), None)?;
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
        let available = match registered(label) {
            Ok(true) => true,
            result => {
                failed |= result.is_err();
                match components(plist, executable, template) {
                    Ok(true) => {
                        commands.push(format!("/bin/launchctl bootstrap system '{plist}'"));
                        true
                    }
                    Ok(false) => false,
                    Err(_) => {
                        failed = true;
                        false
                    }
                }
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
                |_, _, _| Ok(false),
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
    safe_file(path, None).ok()?;
    let source = std::fs::read_to_string(path).ok()?;
    if source.lines().any(|line| {
        line.strip_prefix("TARGET=")
            .is_some_and(|value| value.trim_matches('"') == timezone)
    }) {
        return None;
    }
    Some(format!(
        "/usr/bin/sed -i '' 's|^TARGET=.*|TARGET={timezone}|' '/Library/Application Support/claude-switcher/tz-watchdog.sh'"
    ))
}
