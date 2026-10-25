use crate::policy::SystemActions;
use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use switcher_domain::system::*;

mod dns_cache;
mod timezone_guard;
pub use timezone_guard::require_idle_timezone_guard;

pub struct Platform;
fn command(program: &str, args: &[&str]) -> Result<String, &'static str> {
    #[cfg(test)]
    if let Some(result) = dns_cache::test_io::command(program, args) {
        return result;
    }
    #[cfg(test)]
    if let Some(result) = no_tun_test_io::command(program, args) {
        return result;
    }
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "system_command_spawn")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err("system_command_failed");
                }
                let output = child
                    .wait_with_output()
                    .map_err(|_| "system_command_output")?;
                if output.stdout.len() > 65536 {
                    return Err("system_command_output_large");
                }
                return String::from_utf8(output.stdout).map_err(|_| "system_command_encoding");
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("system_command_timeout");
            }
        }
    }
}
pub fn validate_timezone(zone: &str) -> Result<(), &'static str> {
    if zone.is_empty()
        || zone.len() > 128
        || zone.starts_with('/')
        || zone
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || !zone
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
    {
        return Err("timezone_invalid");
    }
    let root = Path::new("/usr/share/zoneinfo")
        .canonicalize()
        .map_err(|_| "timezone_database")?;
    let path = root
        .join(zone)
        .canonicalize()
        .map_err(|_| "timezone_unknown")?;
    if !path.starts_with(root) || !path.is_file() {
        return Err("timezone_invalid");
    }
    Ok(())
}
fn route_uses_tunnel(output: &str) -> bool {
    let mut interfaces = output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("interface:"))
        .map(str::trim);
    matches!((interfaces.next(), interfaces.next()), (Some(name), None) if name.strip_prefix("utun").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())))
}
fn require_tunnel() -> Result<(), &'static str> {
    let route = command("/sbin/route", &["-n", "get", "1.1.1.1"])?;
    if route_uses_tunnel(&route) {
        Ok(())
    } else {
        Err("tun_route_required")
    }
}
impl SystemActions for Platform {
    fn reconcile_managed_timezone(&self, expected: &str) -> Result<HelperResponse, &'static str> {
        timezone_guard::reconcile(expected)
    }
    fn validate_repair_environment(&self, target: &RegionPolicy) -> Result<(), &'static str> {
        require_tunnel()?;
        self.validate_policy(target)
    }
    fn consume_authorization(&self, form: &[u8]) -> Result<(), &'static str> {
        #[cfg(target_os = "macos")]
        {
            crate::native_authorization::consume(form)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = form;
            Err("macos_required")
        }
    }
    fn random_token(&self) -> Result<String, &'static str> {
        let mut bytes = [0u8; 32];
        if unsafe { libc::getentropy(bytes.as_mut_ptr().cast(), bytes.len()) } != 0 {
            return Err("entropy_unavailable");
        }
        Ok(bytes.iter().map(|v| format!("{v:02x}")).collect())
    }
    fn validate_policy(&self, p: &RegionPolicy) -> Result<(), &'static str> {
        validate_timezone(&p.timezone)?;
        if p.network_service.is_empty()
            || p.network_service.starts_with('-')
            || p.network_service.len() > 128
            || p.network_service.chars().any(char::is_control)
        {
            return Err("network_service_invalid");
        }
        let services = command("/usr/sbin/networksetup", &["-listallnetworkservices"])?;
        if !services
            .lines()
            .skip(1)
            .any(|s| s.trim_start_matches('*') == p.network_service)
        {
            return Err("network_service_unknown");
        }
        Ok(())
    }
    fn read_state(&self, service: &str) -> Result<SystemState, &'static str> {
        // Validate the service against the system inventory before using it as an argument.
        self.validate_policy(&RegionPolicy {
            timezone: "UTC".into(),
            network_service: service.into(),
            ipv6: Ipv6Policy::Off,
        })?;
        let zone = command("/usr/sbin/systemsetup", &["-gettimezone"])?;
        let timezone = zone
            .trim()
            .strip_prefix("Time Zone: ")
            .ok_or("timezone_unreadable")?
            .to_string();
        let info = command("/usr/sbin/networksetup", &["-getinfo", service])?;
        let ipv6 = if info.lines().any(|s| s == "IPv6: Off") {
            Ipv6Policy::Off
        } else if info.lines().any(|s| s == "IPv6: Automatic") {
            Ipv6Policy::Automatic
        } else {
            return Err("ipv6_unreadable");
        };
        Ok(SystemState {
            timezone,
            network_service: service.into(),
            ipv6,
        })
    }
    fn prepare_plan(&self, plan: &RepairPlan) -> Result<(), &'static str> {
        if read_repair_file::<RepairPlan>("repair-journal.json")?.is_some() {
            return Err("repair_recovery_required");
        }
        let path = repair_result_path(&plan.id, true)?;
        write_completion_at(&path, plan, "repair_not_started", false, repair_owner()?)
    }
    fn plan_not_started(&self, plan: &RepairPlan) -> Result<(), &'static str> {
        #[cfg(test)]
        if no_tun_test_io::not_started(plan) {
            return Ok(());
        }
        if read_repair_file::<RepairPlan>("repair-journal.json")?.is_some() {
            return Err("repair_recovery_required");
        }
        remember_repair_completion(plan, "repair_not_started")
    }
    fn recover_repair(&self, plan_id: &str) -> Result<HelperResponse, &'static str> {
        // Validate the durable owner binding before any restoration write.
        read_repair_status(plan_id)?;
        let pending: Option<RepairPlan> = read_repair_file("repair-journal.json")?;
        if let Some(plan) = pending {
            if plan.id != plan_id {
                return Err("repair_plan_id");
            }
            recover_pending_repair()?;
        }
        read_repair_status(plan_id)
    }
    fn repair_status(&self, plan_id: &str) -> Result<HelperResponse, &'static str> {
        read_repair_status(plan_id)
    }
    fn apply_plan(&self, p: &RepairPlan) -> Vec<RepairItemResult> {
        if let Err(code) = require_tunnel() {
            let code = self.plan_not_started(p).err().unwrap_or(code);
            return vec![RepairItemResult {
                item: "tun_route".into(),
                changed: false,
                restored: false,
                error: Some(code.into()),
            }];
        }
        if let Err(code) = persist_repair(p) {
            return vec![RepairItemResult {
                item: "transaction".into(),
                changed: false,
                restored: false,
                error: Some(code.into()),
            }];
        }
        let mut results = Vec::new();
        for (item, changed) in [
            ("timezone", p.original.timezone != p.target.timezone),
            ("ipv6", p.original.ipv6 != p.target.ipv6),
        ] {
            if !changed {
                results.push(RepairItemResult {
                    item: item.into(),
                    changed: false,
                    restored: false,
                    error: None,
                });
                continue;
            }
            let result = require_tunnel().and_then(|()| {
                if item == "timezone" {
                    command(
                        "/usr/sbin/systemsetup",
                        &["-settimezone", &p.target.timezone],
                    )
                } else {
                    command(
                        "/usr/sbin/networksetup",
                        &[
                            if p.target.ipv6 == Ipv6Policy::Off {
                                "-setv6off"
                            } else {
                                "-setv6automatic"
                            },
                            &p.target.network_service,
                        ],
                    )
                }
            });
            results.push(RepairItemResult {
                item: item.into(),
                changed: result.is_ok(),
                restored: false,
                error: result.err().map(str::to_string),
            });
            if results.last().is_some_and(|r| r.error.is_some()) {
                break;
            }
        }
        let verified = results.iter().all(|r| r.error.is_none())
            && self
                .read_state(&p.target.network_service)
                .is_ok_and(|s| s.timezone == p.target.timezone && s.ipv6 == p.target.ipv6);
        // A cache refresh is transient. On failure retain the journal and report the
        // already-applied system items; recovery can restore configuration, not cache contents.
        if verified && !dns_cache::refresh_after_changes(&mut results, command) {
            return results;
        }
        if verified && let Err(code) = remember_managed(p) {
            results.push(RepairItemResult {
                item: "system_backup".into(),
                changed: false,
                restored: false,
                error: Some(code.into()),
            });
            return results;
        }
        if !verified {
            for result in &mut results {
                if result.changed {
                    let rollback = if result.item == "timezone" {
                        command(
                            "/usr/sbin/systemsetup",
                            &["-settimezone", &p.original.timezone],
                        )
                    } else {
                        command(
                            "/usr/sbin/networksetup",
                            &[
                                if p.original.ipv6 == Ipv6Policy::Off {
                                    "-setv6off"
                                } else {
                                    "-setv6automatic"
                                },
                                &p.original.network_service,
                            ],
                        )
                    };
                    result.restored = rollback.is_ok();
                }
                if result.error.is_none() {
                    result.error = Some("repair_verification_failed".into());
                }
            }
        }
        let restored = self
            .read_state(&p.original.network_service)
            .is_ok_and(|s| s.timezone == p.original.timezone && s.ipv6 == p.original.ipv6);
        if (verified || restored)
            && remember_repair_completion(
                p,
                if verified {
                    "repair_applied"
                } else {
                    "repair_restored"
                },
            )
            .and_then(|()| clear_repair())
            .is_err()
        {
            results.push(RepairItemResult {
                item: "transaction".into(),
                changed: false,
                restored: false,
                error: Some("repair_journal_cleanup".into()),
            });
        }
        results
    }
    fn restore_managed_system(&self) -> Result<(), &'static str> {
        restore_managed(self)
    }
    fn commit_installation(&self, uid: u32) -> Result<(), &'static str> {
        use std::os::unix::fs::MetadataExt;
        self.guardian_status(uid)?;
        let base = Path::new(SYSTEM_ROOT);
        let backup = base.join("component-backup");
        let metadata =
            std::fs::symlink_metadata(&backup).map_err(|_| "installation_backup_missing")?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
            return Err("installation_backup_permissions");
        }
        let allowed = [
            "local.claudeswitcher.helper",
            "local.claudeswitcher.helper.plist",
            "local.claudeswitcher.guardian.plist",
            "switcher-guardian",
            "install.json",
        ];
        for entry in std::fs::read_dir(&backup).map_err(|_| "installation_backup_read")? {
            let entry = entry.map_err(|_| "installation_backup_read")?;
            let name = entry.file_name();
            let metadata =
                std::fs::symlink_metadata(entry.path()).map_err(|_| "installation_backup_read")?;
            if !allowed.iter().any(|v| name == *v)
                || !metadata.is_file()
                || metadata.uid() != 0
                || metadata.mode() & 0o022 != 0
            {
                return Err("installation_backup_permissions");
            }
        }
        let record = load_root_record(Path::new(INSTALL_RECORD))?;
        if record.owner_uid != uid {
            return Err("installation_owner_uid");
        }
        write_component_record_at(base, &record)
    }
    fn guardian_status(&self, uid: u32) -> Result<(), &'static str> {
        command(
            "/bin/launchctl",
            &["print", &format!("gui/{uid}/{GUARDIAN_LABEL}")],
        )
        .map(|_| ())
    }
}

fn write_component_record_at(base: &Path, record: &InstallRecord) -> Result<(), &'static str> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let temporary = base.join(".component-committed.next");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)
        .map_err(|_| "installation_commit_create")?;
    #[cfg(test)]
    f11_test_io::checkpoint("created", &temporary);
    let bytes = serde_json::to_vec(&record).map_err(|_| "installation_commit_encode")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "installation_commit_write")?;
    #[cfg(test)]
    f11_test_io::checkpoint("written", &temporary);
    #[cfg(test)]
    f11_test_io::checkpoint("before_commit", &temporary);
    std::fs::rename(temporary, base.join("component-committed.json"))
        .map_err(|_| "installation_commit_rename")?;
    std::fs::File::open(base)
        .and_then(|f| f.sync_all())
        .map_err(|_| "installation_commit_sync")?;
    // The marker is the commit point. Keep the rollback snapshot until the next transaction.
    let _ = std::fs::remove_file(base.join("component-pending"));
    Ok(())
}

pub fn load_root_record(path: &Path) -> Result<InstallRecord, &'static str> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let mut parent = path.parent();
    while let Some(p) = parent {
        let m = std::fs::symlink_metadata(p).map_err(|_| "install_record_parent")?;
        if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Err("install_record_parent_permissions");
        }
        parent = p.parent();
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "install_record_missing")?;
    let m = file.metadata().map_err(|_| "install_record_metadata")?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o022 != 0 || m.len() > 16384 {
        return Err("install_record_permissions");
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "install_record_read")?;
    let record: InstallRecord =
        serde_json::from_slice(&bytes).map_err(|_| "install_record_invalid")?;
    record.validate()?;
    Ok(record)
}
#[cfg(test)]
mod tests {
    #[test]
    fn preview_receipt_survives_client_crash_and_cannot_overwrite_terminal_receipt() {
        use super::*;
        let directory = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("switcher-repair-receipt-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("plan.json");
        let plan = RepairPlan {
            id: "plan".into(),
            original: SystemState {
                timezone: "UTC".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Automatic,
            },
            target: RegionPolicy {
                timezone: "Asia/Tokyo".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Off,
            },
            expires_at: 30,
        };
        write_completion_at(&path, &plan, "repair_not_started", false, 501).unwrap();
        // Reload without any session/receipt memory, as after region SystemPending then process loss.
        let loaded: RepairCompletion =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(loaded.code, "repair_not_started");
        assert_eq!(loaded.plan.id, plan.id);
        // A crash before the old rename must not prevent the next recovery from committing.
        std::fs::write(path.with_extension("next"), b"interrupted write").unwrap();
        write_completion_at(&path, &plan, "repair_applied", true, 501).unwrap();
        assert_eq!(
            write_completion_at(&path, &plan, "repair_not_started", false, 501),
            Err("repair_completion_create")
        );
        let terminal: RepairCompletion =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(terminal.code, "repair_applied");
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn actual_route_requires_one_real_tunnel_interface() {
        assert!(super::route_uses_tunnel(
            "route to: 1.1.1.1\n interface: utun12\n"
        ));
        for route in [
            "",
            "interface: en0",
            "interface: utun",
            "interface: utun2evil",
            "gateway: utun3",
            "interface: utun2\ninterface: en0",
        ] {
            assert!(!super::route_uses_tunnel(route));
        }
    }
    #[test]
    fn status_requires_matching_plan_and_pending_overrides_completed_receipt() {
        use super::*;
        struct ReadOnly;
        impl SystemActions for ReadOnly {
            fn random_token(&self) -> Result<String, &'static str> {
                panic!("read only")
            }
            fn validate_policy(&self, _: &RegionPolicy) -> Result<(), &'static str> {
                Ok(())
            }
            fn read_state(&self, service: &str) -> Result<SystemState, &'static str> {
                Ok(SystemState {
                    timezone: "UTC".into(),
                    network_service: service.into(),
                    ipv6: Ipv6Policy::Automatic,
                })
            }
            fn apply_plan(&self, _: &RepairPlan) -> Vec<RepairItemResult> {
                panic!("read only")
            }
            fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
                panic!("read only")
            }
        }
        let plan = RepairPlan {
            id: "plan".into(),
            original: SystemState {
                timezone: "UTC".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Automatic,
            },
            target: RegionPolicy {
                timezone: "Asia/Tokyo".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Off,
            },
            expires_at: 100,
        };
        let receipt = || {
            Some(RepairCompletion {
                owner_uid: 501,
                code: "repair_applied".into(),
                plan: plan.clone(),
            })
        };
        assert_eq!(
            status_response("plan", Some(plan.clone()), receipt(), &ReadOnly, 501)
                .unwrap()
                .code,
            "repair_recovery_required"
        );
        assert_eq!(
            status_response("other", None, receipt(), &ReadOnly, 501)
                .unwrap()
                .code,
            "repair_unknown"
        );
        assert!(matches!(
            status_response("plan", Some(plan.clone()), receipt(), &ReadOnly, 502),
            Err("repair_result_owner")
        ));
        let applied = status_response("plan", None, receipt(), &ReadOnly, 501).unwrap();
        assert_eq!(applied.code, "repair_applied");
        assert_eq!(applied.state.unwrap().timezone, "UTC"); // actual, never a fabricated target
        assert_eq!(
            status_response("plan", None, None, &ReadOnly, 501)
                .unwrap()
                .code,
            "repair_unknown"
        );
        let rejected = status_response(
            "plan",
            None,
            Some(RepairCompletion {
                owner_uid: 501,
                code: "repair_not_started".into(),
                plan,
            }),
            &ReadOnly,
            501,
        )
        .unwrap();
        assert_eq!(rejected.code, "repair_not_started");
        assert!(!rejected.ok);
    }

    use super::*;
    #[test]
    fn timezone_rejects_injection_and_traversal() {
        for z in [
            "../../etc/passwd",
            "/etc/passwd",
            "UTC;id",
            "UTC\n",
            "America//New_York",
        ] {
            assert!(validate_timezone(z).is_err());
        }
        assert!(validate_timezone("UTC").is_ok());
    }
    #[test]
    fn entropy_is_fresh() {
        let p = Platform;
        let a = p.random_token().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, p.random_token().unwrap());
    }
}

fn persist_repair(plan: &RepairPlan) -> Result<(), &'static str> {
    #[cfg(test)]
    if dns_cache::test_io::journal("persist", None) {
        return Ok(());
    }
    #[cfg(test)]
    no_tun_test_io::persist_tripwire();
    persist_repair_at(Path::new(SYSTEM_ROOT), plan)
}
fn persist_repair_at(base: &Path, plan: &RepairPlan) -> Result<(), &'static str> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let path = base.join("repair-journal.json");
    if path.exists() {
        return Err("repair_recovery_required");
    }
    let temporary = base.join(".repair-journal.next");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)
        .map_err(|_| "repair_journal_create")?;
    #[cfg(test)]
    f11_test_io::checkpoint("created", &temporary);
    let bytes = serde_json::to_vec(plan).map_err(|_| "repair_journal_encode")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "repair_journal_write")?;
    #[cfg(test)]
    f11_test_io::checkpoint("written", &temporary);
    #[cfg(test)]
    f11_test_io::checkpoint("before_commit", &temporary);
    std::fs::rename(temporary, path).map_err(|_| "repair_journal_commit")?;
    std::fs::File::open(base)
        .and_then(|f| f.sync_all())
        .map_err(|_| "repair_journal_sync")
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RepairCompletion {
    owner_uid: u32,
    code: String,
    plan: RepairPlan,
}
fn repair_result_path(plan_id: &str, create: bool) -> Result<std::path::PathBuf, &'static str> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    if plan_id.len() != 64 || !plan_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("repair_plan_id");
    }
    let directory = Path::new(SYSTEM_ROOT).join("repair-results");
    if create {
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(_) => return Err("repair_results_directory"),
        }
    }
    if create {
        std::fs::File::open(SYSTEM_ROOT)
            .and_then(|file| file.sync_all())
            .map_err(|_| "repair_results_sync")?;
    }
    let m = std::fs::symlink_metadata(&directory).map_err(|_| "repair_results_directory")?;
    if !m.is_dir() || m.file_type().is_symlink() || m.uid() != 0 || m.mode() & 0o077 != 0 {
        return Err("repair_results_permissions");
    }
    Ok(directory.join(format!("{plan_id}.json")))
}
fn write_completion_at(
    path: &Path,
    plan: &RepairPlan,
    code: &str,
    replace: bool,
    owner_uid: u32,
) -> Result<(), &'static str> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    // A unique private temporary inode avoids a stale pre-rename file blocking recovery.
    let temp = path.with_extension(format!("{}.next", Platform.random_token()?));
    let target = if replace { temp.as_path() } else { path };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(target)
        .map_err(|_| "repair_completion_create")?;
    let data = serde_json::to_vec(&RepairCompletion {
        owner_uid,
        code: code.into(),
        plan: plan.clone(),
    })
    .map_err(|_| "repair_completion_encoding")?;
    file.write_all(&data)
        .and_then(|()| file.sync_all())
        .map_err(|_| "repair_completion_write")?;
    if replace {
        std::fs::rename(temp, path).map_err(|_| "repair_completion_commit")?;
    }
    std::fs::File::open(path.parent().ok_or("repair_results_directory")?)
        .and_then(|f| f.sync_all())
        .map_err(|_| "repair_completion_sync")
}
fn repair_owner() -> Result<u32, &'static str> {
    Ok(load_root_record(Path::new(INSTALL_RECORD))?.owner_uid)
}
fn remember_repair_completion(plan: &RepairPlan, code: &str) -> Result<(), &'static str> {
    #[cfg(test)]
    if dns_cache::test_io::journal("completion", Some(code)) {
        return Ok(());
    }
    let path = repair_result_path(&plan.id, true)?;
    write_completion_at(&path, plan, code, true, repair_owner()?)
}
fn read_repair_file<T: serde::de::DeserializeOwned>(name: &str) -> Result<Option<T>, &'static str> {
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(Path::new(SYSTEM_ROOT).join(name))
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("repair_status_read"),
    };
    let m = file.metadata().map_err(|_| "repair_status_metadata")?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 || m.nlink() != 1 || m.len() > 16384 {
        return Err("repair_status_permissions");
    }
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "repair_status_read")?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "repair_status_schema")
}
fn status_response(
    plan_id: &str,
    pending: Option<RepairPlan>,
    completion: Option<RepairCompletion>,
    system: &impl SystemActions,
    owner_uid: u32,
) -> Result<HelperResponse, &'static str> {
    if completion
        .as_ref()
        .is_some_and(|receipt| receipt.owner_uid != owner_uid)
        || (pending.is_some() && completion.is_none())
    {
        return Err("repair_result_owner");
    }
    if completion
        .as_ref()
        .is_some_and(|receipt| receipt.plan.id != plan_id)
    {
        return Ok(HelperResponse::status("repair_unknown", false));
    }
    let (code, plan) = if let Some(plan) = pending {
        ("repair_recovery_required", plan)
    } else if let Some(receipt) = completion {
        if !matches!(
            receipt.code.as_str(),
            "repair_applied" | "repair_restored" | "repair_not_started"
        ) {
            return Err("repair_status_schema");
        }
        (
            if receipt.code == "repair_applied" {
                "repair_applied"
            } else if receipt.code == "repair_not_started" {
                "repair_not_started"
            } else {
                "repair_restored"
            },
            receipt.plan,
        )
    } else {
        return Ok(HelperResponse::status("repair_unknown", false));
    };
    if plan.id != plan_id {
        return Ok(HelperResponse::status("repair_unknown", false));
    }
    let state = system.read_state(&plan.target.network_service)?;
    let mut response = HelperResponse::status(code, code != "repair_not_started");
    response.state = Some(state);
    response.plan = Some(plan);
    Ok(response)
}
fn read_repair_status(plan_id: &str) -> Result<HelperResponse, &'static str> {
    if plan_id.len() != 64 || !plan_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("repair_plan_id");
    }
    let pending = read_repair_file("repair-journal.json")?;
    let path = repair_result_path(plan_id, false)?;
    let name = path
        .strip_prefix(SYSTEM_ROOT)
        .map_err(|_| "repair_results_directory")?
        .to_str()
        .ok_or("repair_plan_id")?;
    let completion = read_repair_file(name)?;
    status_response(plan_id, pending, completion, &Platform, repair_owner()?)
}
fn clear_repair() -> Result<(), &'static str> {
    #[cfg(test)]
    if dns_cache::test_io::journal("clear", None) {
        return Ok(());
    }
    std::fs::remove_file(Path::new(SYSTEM_ROOT).join("repair-journal.json"))
        .map_err(|_| "repair_journal_remove")?;
    std::fs::File::open(SYSTEM_ROOT)
        .and_then(|f| f.sync_all())
        .map_err(|_| "repair_journal_sync")
}
/// Resume only a root-protected repair journal. External changes cause a conflict, never overwrite.
pub fn recover_pending_repair() -> Result<(), &'static str> {
    #[cfg(test)]
    if let Some(pending) = dns_cache::test_io::pending_plan() {
        return pending.as_ref().map_or(Ok(()), recover_saved_plan);
    }
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let path = Path::new(SYSTEM_ROOT).join("repair-journal.json");
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("repair_recovery_read"),
    };
    let m = file.metadata().map_err(|_| "repair_recovery_metadata")?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 || m.len() > 16384 {
        return Err("repair_recovery_permissions");
    }
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "repair_recovery_read")?;
    let p: RepairPlan = serde_json::from_slice(&bytes).map_err(|_| "repair_recovery_schema")?;
    recover_saved_plan(&p)
}
fn recover_saved_plan(p: &RepairPlan) -> Result<(), &'static str> {
    let system = Platform;
    system.validate_policy(&p.target)?;
    validate_timezone(&p.original.timezone)?;
    if p.target.network_service != p.original.network_service {
        return Err("repair_recovery_service");
    }
    let current = system.read_state(&p.target.network_service)?;
    if ![p.original.timezone.as_str(), p.target.timezone.as_str()]
        .contains(&current.timezone.as_str())
        || ![p.original.ipv6, p.target.ipv6].contains(&current.ipv6)
    {
        return Err("repair_recovery_conflict");
    }
    if current.timezone != p.original.timezone {
        command(
            "/usr/sbin/systemsetup",
            &["-settimezone", &p.original.timezone],
        )?;
    }
    if current.ipv6 != p.original.ipv6 {
        command(
            "/usr/sbin/networksetup",
            &[
                if p.original.ipv6 == Ipv6Policy::Off {
                    "-setv6off"
                } else {
                    "-setv6automatic"
                },
                &p.original.network_service,
            ],
        )?;
    }
    let restored = system.read_state(&p.original.network_service)?;
    if restored.timezone != p.original.timezone || restored.ipv6 != p.original.ipv6 {
        return Err("repair_recovery_verification");
    }
    remember_repair_completion(p, "repair_restored")?;
    clear_repair()
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedSystem {
    timezone_original: String,
    timezone_last: String,
    #[serde(default)]
    timezone_guard_plan_id: Option<String>,
    services: std::collections::BTreeMap<String, (Ipv6Policy, Ipv6Policy)>,
}
fn read_managed() -> Result<Option<ManagedSystem>, &'static str> {
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let path = Path::new(SYSTEM_ROOT).join("system-original.json");
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("system_backup_read"),
    };
    let m = file.metadata().map_err(|_| "system_backup_metadata")?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 || m.len() > 16384 {
        return Err("system_backup_permissions");
    }
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "system_backup_read")?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "system_backup_schema")
}
fn remember_managed(plan: &RepairPlan) -> Result<(), &'static str> {
    #[cfg(test)]
    if dns_cache::test_io::journal("managed", None) {
        return Ok(());
    }
    let mut state = read_managed()?.unwrap_or(ManagedSystem {
        timezone_original: plan.original.timezone.clone(),
        timezone_last: plan.original.timezone.clone(),
        timezone_guard_plan_id: None,
        services: Default::default(),
    });
    state.timezone_last = plan.target.timezone.clone();
    state.timezone_guard_plan_id = Some(plan.id.clone());
    let item = state
        .services
        .entry(plan.target.network_service.clone())
        .or_insert((plan.original.ipv6, plan.original.ipv6));
    item.1 = plan.target.ipv6;
    write_managed_record_at(Path::new(SYSTEM_ROOT), &state)
}
fn write_managed_record_at(base: &Path, state: &ManagedSystem) -> Result<(), &'static str> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let temporary = base.join(".system-original.next");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)
        .map_err(|_| "system_backup_create")?;
    #[cfg(test)]
    f11_test_io::checkpoint("created", &temporary);
    let bytes = serde_json::to_vec(&state).map_err(|_| "system_backup_encode")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "system_backup_write")?;
    #[cfg(test)]
    f11_test_io::checkpoint("written", &temporary);
    #[cfg(test)]
    f11_test_io::checkpoint("before_commit", &temporary);
    std::fs::rename(temporary, base.join("system-original.json"))
        .map_err(|_| "system_backup_commit")?;
    std::fs::File::open(base)
        .and_then(|f| f.sync_all())
        .map_err(|_| "system_backup_sync")
}
fn restore_managed(system: &Platform) -> Result<(), &'static str> {
    let Some(state) = read_managed()? else {
        return Ok(());
    };
    validate_timezone(&state.timezone_original)?;
    validate_timezone(&state.timezone_last)?;
    if state.services.is_empty() || state.services.len() > 32 {
        return Err("system_backup_services");
    }
    let mut originals = Vec::new();
    for (service, (original, last)) in &state.services {
        let current = system.read_state(service)?;
        if current.timezone != state.timezone_last && current.timezone != state.timezone_original {
            return Err("uninstall_timezone_conflict");
        }
        if current.ipv6 != *last && current.ipv6 != *original {
            return Err("uninstall_ipv6_conflict");
        }
        originals.push((current, *original));
    }
    for (current, ipv6) in originals {
        let plan = RepairPlan {
            id: system.random_token()?,
            original: current.clone(),
            target: RegionPolicy {
                timezone: state.timezone_original.clone(),
                network_service: current.network_service.clone(),
                ipv6,
            },
            expires_at: 0,
        };
        persist_repair(&plan)?;
        if current.timezone != plan.target.timezone {
            command(
                "/usr/sbin/systemsetup",
                &["-settimezone", &plan.target.timezone],
            )?;
        }
        if current.ipv6 != ipv6 {
            command(
                "/usr/sbin/networksetup",
                &[
                    if ipv6 == Ipv6Policy::Off {
                        "-setv6off"
                    } else {
                        "-setv6automatic"
                    },
                    &current.network_service,
                ],
            )?;
        }
        let verified = system.read_state(&current.network_service)?;
        if verified.timezone != plan.target.timezone || verified.ipv6 != ipv6 {
            return Err("uninstall_restore_verification");
        }
        clear_repair()?;
    }
    std::fs::remove_file(Path::new(SYSTEM_ROOT).join("system-original.json"))
        .map_err(|_| "system_backup_remove")?;
    std::fs::File::open(SYSTEM_ROOT)
        .and_then(|f| f.sync_all())
        .map_err(|_| "system_backup_sync")
}

#[cfg(test)]
mod f11_test_io {
    use super::*;
    use std::{cell::RefCell, os::unix::fs::MetadataExt, path::PathBuf};

    type Hook = Box<dyn Fn(&str, &Path)>;
    thread_local! {
        static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }
    pub(super) fn checkpoint(stage: &str, path: &Path) {
        HOOK.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(stage, path);
            }
        });
    }
    fn scoped(hook: impl Fn(&str, &Path) + 'static, action: impl FnOnce()) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                HOOK.with(|hook| *hook.borrow_mut() = None);
            }
        }
        HOOK.with(|slot| {
            assert!(slot.borrow().is_none());
            *slot.borrow_mut() = Some(Box::new(hook));
        });
        let _reset = Reset;
        action();
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("helper-f11-{}", Platform.random_token().unwrap()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[derive(Clone, Copy, Debug)]
    enum Writer { Journal, Component, Managed }
    impl Writer {
        fn names(self) -> (&'static str, &'static str) {
            match self {
                Self::Journal => (".repair-journal.next", "repair-journal.json"),
                Self::Component => (".component-committed.next", "component-committed.json"),
                Self::Managed => (".system-original.next", "system-original.json"),
            }
        }
        fn write(self, root: &Path) -> Result<(), &'static str> {
            match self {
                Self::Journal => persist_repair_at(root, &plan()),
                Self::Component => write_component_record_at(root, &InstallRecord {
                    schema_version: 2, owner_uid: 501,
                    edition: switcher_domain::Edition::Personal,
                    trust: InstallTrust::DeveloperId { team_id: "TESTTEAM01".into() },
                    helper_version: "0.2.0".into(), minimum_app_version: "0.2.0".into(),
                }),
                Self::Managed => write_managed_record_at(root, &ManagedSystem {
                    timezone_original: "UTC".into(), timezone_last: "Asia/Tokyo".into(),
                    timezone_guard_plan_id: Some(plan().id),
                    services: [("Wi-Fi".into(), (Ipv6Policy::Automatic, Ipv6Policy::Off))].into(),
                }),
            }
        }
    }
    fn plan() -> RepairPlan {
        RepairPlan {
            id: "a".repeat(64),
            original: SystemState { timezone: "UTC".into(), network_service: "Wi-Fi".into(), ipv6: Ipv6Policy::Automatic },
            target: RegionPolicy { timezone: "Asia/Tokyo".into(), network_service: "Wi-Fi".into(), ipv6: Ipv6Policy::Off },
            expires_at: 100,
        }
    }
    fn retry_interrupted(writer: Writer) {
        for stage in ["created", "partial", "written", "before_commit"] {
            let root = Directory::new();
            let result = std::panic::catch_unwind(|| scoped(move |point, path| {
                if (stage == "partial" && point == "created") || point == stage {
                    if stage == "partial" { std::fs::write(path, b"{").unwrap(); }
                    panic!("synthetic interruption {stage}");
                }
            }, || { writer.write(&root.0).unwrap(); }));
            assert!(result.is_err(), "{writer:?} {stage}");
            let leftovers: Vec<_> = std::fs::read_dir(&root.0).unwrap().map(|p| {
                let path=p.unwrap().path(); let meta=std::fs::symlink_metadata(&path).unwrap();
                (path, meta.dev(), meta.ino(), meta.len())
            }).collect();
            assert_eq!(leftovers.len(), 1);
            assert_eq!(writer.write(&root.0), Ok(()), "{writer:?} {stage}");
            for (path, dev, ino, len) in leftovers {
                let now=std::fs::symlink_metadata(path).unwrap();
                assert_eq!((now.dev(),now.ino(),now.len()), (dev,ino,len));
            }
            let final_path=root.0.join(writer.names().1);
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(final_path).unwrap()).unwrap();
        }
    }
    #[test] fn f11_journal_retries_after_each_interrupted_write() { retry_interrupted(Writer::Journal); }
    #[test] fn f11_component_retries_after_each_interrupted_write() { retry_interrupted(Writer::Component); }
    #[test] fn f11_managed_retries_after_each_interrupted_write() { retry_interrupted(Writer::Managed); }
    #[test]
    fn f11_old_fixed_residues_are_preserved_and_do_not_block_new_writes() {
        use std::os::unix::fs::symlink;
        for writer in [Writer::Journal,Writer::Component,Writer::Managed] {
            for kind in ["regular", "directory", "symlink", "dangling", "hardlink"] {
                let root=Directory::new(); let old=root.0.join(writer.names().0);
                let referent=root.0.join("referent");
                std::fs::write(&referent,b"external data").unwrap();
                match kind {
                    "regular"=>std::fs::write(&old,b"old residue").unwrap(),
                    "directory"=>std::fs::create_dir(&old).unwrap(),
                    "symlink"=>symlink(&referent,&old).unwrap(),
                    "dangling"=>symlink(root.0.join("absent"),&old).unwrap(),
                    "hardlink"=>std::fs::hard_link(&referent,&old).unwrap(), _=>unreachable!()
                }
                let before=std::fs::symlink_metadata(&old).unwrap();
                assert_eq!(writer.write(&root.0),Ok(()),"{writer:?} {kind}");
                let after=std::fs::symlink_metadata(&old).unwrap();
                assert_eq!((before.dev(),before.ino(),before.mode(),before.nlink()),(after.dev(),after.ino(),after.mode(),after.nlink()));
                assert_eq!(std::fs::read(&referent).unwrap(),b"external data");
            }
        }
    }
    #[test]
    fn f11_replaced_temp_is_neither_published_nor_removed() {
        use std::os::unix::fs::symlink;
        for writer in [Writer::Journal,Writer::Component,Writer::Managed] {
            for linked in [false,true] {
                let root=Directory::new(); let referent=root.0.join("referent");
                std::fs::write(&referent,b"external data").unwrap();
                let referent_hook=referent.clone();
                scoped(move |stage,path| {
                    if stage=="before_commit" {
                        std::fs::rename(path,path.with_extension("held")).unwrap();
                        if linked { symlink(&referent_hook,path).unwrap(); }
                        else { std::fs::write(path,b"external replacement").unwrap(); }
                    }
                }, || { assert!(writer.write(&root.0).is_err(),"{writer:?} {linked}"); });
                assert!(!root.0.join(writer.names().1).exists());
                let residues:Vec<_>=std::fs::read_dir(&root.0).unwrap().map(|p|p.unwrap().path()).collect();
                assert_eq!(residues.len(),3);
                assert_eq!(std::fs::read(referent).unwrap(),b"external data");
            }
        }
    }
    #[test]
    fn f11_existing_and_racing_final_journals_are_never_overwritten() {
        use std::os::unix::fs::symlink;
        for kind in ["file", "symlink", "dangling", "directory"] {
            for racing in [false,true] {
                let root=Directory::new(); let path=root.0.join("repair-journal.json");
                let make=move |_:&str, _: &Path| {
                    match kind {
                        "file"=>std::fs::write(&path,b"existing journal").unwrap(),
                        "symlink"=>symlink("referent",&path).unwrap(),
                        "dangling"=>symlink("absent",&path).unwrap(),
                        "directory"=>std::fs::create_dir(&path).unwrap(), _=>unreachable!()
                    }
                };
                std::fs::write(root.0.join("referent"),b"external data").unwrap();
                if racing { scoped(move |stage,temp| {if stage=="before_commit" {make(stage,temp)}}, || assert!(Writer::Journal.write(&root.0).is_err(),"{kind} race")); }
                else { make("",&root.0); assert!(Writer::Journal.write(&root.0).is_err(),"{kind}"); }
                let final_path=root.0.join("repair-journal.json");
                let meta=std::fs::symlink_metadata(&final_path).unwrap();
                match kind {
                    "file"=>assert_eq!(std::fs::read(final_path).unwrap(),b"existing journal"),
                    "symlink"|"dangling"=>assert!(meta.file_type().is_symlink()),
                    "directory"=>assert!(meta.is_dir()), _=>unreachable!()
                }
                assert_eq!(std::fs::read(root.0.join("referent")).unwrap(),b"external data");
            }
        }
    }
}

// These scoped boundaries exist only in the unit-test binary. No environment or IPC switch.
#[cfg(test)]
mod no_tun_test_io {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Case {
        route: Option<Result<String, &'static str>>,
        commands: Vec<(String, Vec<String>)>,
        not_started: Vec<String>,
        persist_attempts: usize,
    }
    thread_local! {
        static CASE: RefCell<Option<Case>> = const { RefCell::new(None) };
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CASE.with(|case| *case.borrow_mut() = None);
        }
    }
    fn scoped(route: Option<Result<String, &'static str>>, run: impl FnOnce()) -> Case {
        CASE.with(|case| {
            assert!(
                case.borrow().is_none(),
                "no-TUN test boundary must not nest"
            );
            *case.borrow_mut() = Some(Case {
                route,
                ..Default::default()
            });
        });
        let _reset = Reset;
        run();
        CASE.with(|case| case.borrow_mut().take().unwrap())
    }
    pub(super) fn command(program: &str, args: &[&str]) -> Option<Result<String, &'static str>> {
        CASE.with(|case| {
            let mut case = case.borrow_mut();
            let case = case.as_mut()?;
            case.commands.push((
                program.into(),
                args.iter().map(|arg| (*arg).into()).collect(),
            ));
            assert_eq!(
                program, "/sbin/route",
                "unexpected command blocked before spawn"
            );
            assert_eq!(
                args,
                ["-n", "get", "1.1.1.1"],
                "unexpected argv blocked before spawn"
            );
            Some(
                case.route
                    .clone()
                    .expect("unconfigured route boundary blocked before spawn"),
            )
        })
    }
    pub(super) fn not_started(plan: &RepairPlan) -> bool {
        CASE.with(|case| {
            let mut case = case.borrow_mut();
            let Some(case) = case.as_mut() else {
                return false;
            };
            case.not_started.push(plan.id.clone());
            true
        })
    }
    pub(super) fn persist_tripwire() {
        CASE.with(|case| {
            if let Some(case) = case.borrow_mut().as_mut() {
                case.persist_attempts += 1;
                panic!("journal tripwire blocked before any filesystem access");
            }
        });
    }
    fn plan() -> RepairPlan {
        RepairPlan {
            id: "a".repeat(64),
            original: SystemState {
                timezone: "UTC".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Automatic,
            },
            target: RegionPolicy {
                timezone: "Asia/Tokyo".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Off,
            },
            expires_at: 100,
        }
    }
    #[test]
    fn no_tun_apply_plan_never_reaches_system_or_journal_writes() {
        let cases = [
            (Ok("interface: en0".into()), "tun_route_required"),
            (Ok(String::new()), "tun_route_required"),
            (Ok("interface: utun".into()), "tun_route_required"),
            (Ok("interface: utun2evil".into()), "tun_route_required"),
            (Ok("gateway: utun3".into()), "tun_route_required"),
            (
                Ok("interface: utun2\ninterface: en0".into()),
                "tun_route_required",
            ),
            (Err("system_command_failed"), "system_command_failed"),
            (Err("system_command_timeout"), "system_command_timeout"),
        ];
        for (route, expected) in cases {
            let plan = plan();
            let case = scoped(Some(route), || {
                let results = Platform.apply_plan(&plan);
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].item, "tun_route");
                assert!(!results[0].changed);
                assert!(!results[0].restored);
                assert_eq!(results[0].error.as_deref(), Some(expected));
            });
            assert_eq!(
                case.commands,
                [(
                    "/sbin/route".into(),
                    vec!["-n".into(), "get".into(), "1.1.1.1".into()]
                )]
            );
            assert_eq!(case.persist_attempts, 0);
            assert_eq!(case.not_started, [plan.id]);
        }
    }
    #[test]
    fn no_tun_positive_control_trips_before_root_journal_io() {
        let case = scoped(Some(Ok("interface: utun12".into())), || {
            let panic = std::panic::catch_unwind(|| Platform.apply_plan(&plan())).unwrap_err();
            assert_eq!(
                panic.downcast_ref::<&str>().copied(),
                Some("journal tripwire blocked before any filesystem access")
            );
        });
        assert_eq!(case.persist_attempts, 1);
        assert_eq!(case.commands.len(), 1);
        assert!(case.not_started.is_empty());
    }
    #[test]
    fn no_tun_missing_boundary_fails_closed_and_unwind_resets_scope() {
        let panic = std::panic::catch_unwind(|| {
            scoped(None, || {
                Platform.apply_plan(&plan());
            });
        });
        assert!(panic.is_err());
        assert!(CASE.with(|case| case.borrow().is_none()));
        let case = scoped(Some(Ok("interface: en0".into())), || {
            Platform.apply_plan(&plan());
        });
        assert_eq!(case.persist_attempts, 0);
        assert_eq!(case.not_started.len(), 1);
    }
    #[test]
    fn no_tun_scope_is_thread_local_and_denies_unexpected_commands() {
        let case = scoped(Some(Ok("interface: en0".into())), || {
            assert!(
                std::thread::spawn(|| CASE.with(|case| case.borrow().is_none()))
                    .join()
                    .unwrap()
            );
            assert!(
                std::panic::catch_unwind(|| {
                    command("/usr/sbin/systemsetup", &["-settimezone", "Asia/Tokyo"]);
                })
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(|| {
                    command("/usr/sbin/networksetup", &["-setv6off", "Wi-Fi"]);
                })
                .is_err()
            );
        });
        assert_eq!(case.commands.len(), 2);
        assert_eq!(case.persist_attempts, 0);
        assert!(case.not_started.is_empty());
    }
}
