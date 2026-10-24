//! Root-only, fixed operation runner. Its inherited flock outlives the GUI and its own parent.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::Command,
};
use switcher_domain::{
    Edition,
    system::{INSTALL_RECORD, SYSTEM_ROOT},
};
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RootOperation {
    InstallPackage {
        source: PathBuf,
        team: String,
        window: Option<RootWindow>,
        sha256: Option<String>,
    },
    ReplaceApp {
        source: PathBuf,
        edition: Edition,
        team: String,
        old: String,
        new: String,
        window: RootWindow,
        cdhash: String,
    },
    RestoreApp {
        edition: Edition,
        team: String,
        old: String,
        new: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootWindow {
    pub issued: u64,
    pub expires: u64,
    pub utc: i64,
    pub continuous_ms: u64,
    pub boot_id: String,
}
impl RootWindow {
    fn validate(&self) -> Result<(), &'static str> {
        #[repr(C)]
        struct Timebase {
            n: u32,
            d: u32,
        }
        unsafe extern "C" {
            fn mach_continuous_time() -> u64;
            fn mach_timebase_info(i: *mut Timebase) -> i32;
        }
        let mut t = Timebase { n: 0, d: 0 };
        if unsafe { mach_timebase_info(&mut t) } != 0 || t.d == 0 {
            return Err("root_clock");
        }
        let continuous = (u128::from(unsafe { mach_continuous_time() }) * u128::from(t.n)
            / u128::from(t.d)
            / 1_000_000)
            .try_into()
            .map_err(|_| "root_clock")?;
        let utc = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "root_clock")?
            .as_secs();
        self.validate_observation(utc, continuous, &boot_id()?)
    }
    fn validate_observation(
        &self,
        utc: u64,
        continuous: u64,
        boot: &str,
    ) -> Result<(), &'static str> {
        if boot != self.boot_id || continuous < self.continuous_ms {
            return Err("clock_recovery_required");
        }
        let elapsed = i128::from(continuous - self.continuous_ms);
        if ((i128::from(utc) - i128::from(self.utc)) * 1000 - elapsed).abs() > 300_000 {
            return Err("clock_recovery_required");
        }
        if utc < self.issued || utc >= self.expires {
            return Err("metadata_expired");
        }
        Ok(())
    }
}
fn ensure_claude_stopped(owner: u32) -> Result<(), &'static str> {
    let out = Command::new("/bin/ps")
        .args(["-ww", "-axo", "uid=,comm="])
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .map_err(|_| "process_inventory")?;
    if !out.status.success() || out.stdout.len() > 1024 * 1024 {
        return Err("process_inventory");
    }
    let text = std::str::from_utf8(&out.stdout).map_err(|_| "process_inventory")?;
    for line in text.lines() {
        let line = line.trim_start();
        let at = line.find(char::is_whitespace).ok_or("process_inventory")?;
        let uid: u32 = line[..at].parse().map_err(|_| "process_inventory")?;
        let name = line[at..].trim();
        if uid == owner && (name.contains("/Claude.app/Contents/") || name == "Claude") {
            return Err("claude_running");
        }
    }
    Ok(())
}
pub struct RootLock(File);
impl RootLock {
    pub fn acquire() -> Result<Self, &'static str> {
        if unsafe { libc::geteuid() } != 0 {
            return Err("root_required");
        }
        safe_root(Path::new(SYSTEM_ROOT))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(Path::new(SYSTEM_ROOT).join("root-operation.lock"))
            .map_err(|_| "root_transaction_lock")?;
        let meta = file.metadata().map_err(|_| "root_transaction_lock")?;
        if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o077 != 0 || meta.nlink() != 1 {
            return Err("root_transaction_lock_type");
        }
        let lock = Self::lock(file)?;
        check_source_local_transaction(Path::new(SYSTEM_ROOT))?;
        check_installer_barrier()?;
        Ok(lock)
    }
    fn lock(file: File) -> Result<Self, &'static str> {
        // Keep this descriptor through exec: installer/ditto retain the same open-file lock
        // even if the GUI and this runner both die. A successor must acquire this lock too.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("root_transaction_busy");
        }
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) } != 0 {
            return Err("root_transaction_fd");
        }
        Ok(Self(file))
    }
    pub fn held_fd(&self) -> i32 {
        self.0.as_raw_fd()
    }
}
pub(crate) fn boot_id() -> Result<String, &'static str> {
    let out = Command::new("/usr/sbin/sysctl")
        .args(["-n", "kern.bootsessionuuid"])
        .output()
        .map_err(|_| "root_boot_id")?;
    let id = String::from_utf8(out.stdout)
        .map_err(|_| "root_boot_id")?
        .trim()
        .to_owned();
    if !out.status.success()
        || id.len() != 36
        || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
    {
        return Err("root_boot_id");
    }
    Ok(id)
}
/// Only the independent local root installer may recover or remove this journal.
/// Presence and metadata errors block ordinary root operations without reading untrusted contents.
pub fn source_local_pending() -> Result<bool, &'static str> {
    match check_source_local_transaction(Path::new(SYSTEM_ROOT)) {
        Ok(()) => Ok(false),
        Err("source_local_recovery_required") => Ok(true),
        Err(error) => Err(error),
    }
}
fn check_source_local_transaction(root: &Path) -> Result<(), &'static str> {
    match fs::symlink_metadata(root.join("source-local-transaction.json")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err("source_local_recovery_required"),
    }
}
fn barrier_path() -> PathBuf {
    Path::new(SYSTEM_ROOT).join("component-installer-active")
}
fn check_installer_barrier() -> Result<(), &'static str> {
    use std::io::Read;
    let path = barrier_path();
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("root_installer_active"),
    };
    let m = file.metadata().map_err(|_| "root_installer_active")?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o022 != 0 || m.len() > 64 {
        return Err("root_installer_active");
    }
    let mut id = String::new();
    file.take(65)
        .read_to_string(&mut id)
        .map_err(|_| "root_installer_active")?;
    if id.trim() == boot_id()? {
        return Err("root_installer_active");
    }
    // A prior boot's installd job cannot still own the filesystem. Its root backup remains.
    fs::remove_file(path).map_err(|_| "root_installer_barrier_clear")
}
fn start_installer_barrier() -> Result<(), &'static str> {
    use std::io::Write;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW)
        .open(barrier_path())
        .map_err(|_| "root_installer_barrier")?;
    file.write_all(boot_id()?.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| "root_installer_barrier")?;
    File::open(SYSTEM_ROOT)
        .and_then(|f| f.sync_all())
        .map_err(|_| "root_installer_barrier")
}
pub(crate) fn safe_root(path: &Path) -> Result<(), &'static str> {
    for parent in path.ancestors() {
        let m = fs::symlink_metadata(parent).map_err(|_| "root_directory")?;
        if !m.is_dir() || m.file_type().is_symlink() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Err("root_directory_permissions");
        }
    }
    Ok(())
}
fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), &'static str> {
    let status = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .status()
        .map_err(|_| "root_command_spawn")?;
    if status.success() {
        Ok(())
    } else {
        Err("root_command_failed")
    }
}
fn team_valid(team: &str) -> bool {
    team.len() == 10
        && team
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}
fn version_valid(v: &str) -> bool {
    let p: Vec<_> = v.split('.').collect();
    p.len() == 3
        && p.iter()
            .all(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))
}
fn app_path(edition: Edition) -> PathBuf {
    PathBuf::from(match edition {
        Edition::Personal => "/Applications/Claude 切换器 Personal.app",
        Edition::Public => "/Applications/Claude 切换器 Public.app",
    })
}
fn backup_path(edition: Edition) -> PathBuf {
    Path::new(SYSTEM_ROOT).join(match edition {
        Edition::Personal => "app-Personal-previous.app",
        Edition::Public => "app-Public-previous.app",
    })
}
fn bundle(edition: Edition) -> &'static str {
    match edition {
        Edition::Personal => "local.claudeswitcher.personal",
        Edition::Public => "local.claudeswitcher.public",
    }
}
fn app_signature_args(path: &Path, edition: Edition, team: &str) -> Vec<std::ffi::OsString> {
    let requirement = format!(
        "=anchor apple generic and certificate leaf[subject.OU] = \"{team}\" and identifier \"{}\"",
        bundle(edition)
    );
    [
        "--verify",
        "--deep",
        "--strict",
        "--test-requirement",
        &requirement,
    ]
    .into_iter()
    .map(std::ffi::OsString::from)
    .chain(std::iter::once(path.as_os_str().to_owned()))
    .collect()
}
#[cfg(test)]
#[test]
fn root_codesign_inline_requirement_argv() {
    let path = Path::new("/isolated path/切换器.app");
    for (edition, identifier) in [
        (Edition::Personal, "local.claudeswitcher.personal"),
        (Edition::Public, "local.claudeswitcher.public"),
    ] {
        assert_eq!(app_signature_args(path, edition, "ABCDEFGHIJ"), vec![
            std::ffi::OsString::from("--verify"), "--deep".into(), "--strict".into(), "--test-requirement".into(),
            format!("=anchor apple generic and certificate leaf[subject.OU] = \"ABCDEFGHIJ\" and identifier \"{identifier}\"").into(),
            path.as_os_str().to_owned(),
        ]);
    }
}

fn verify_app(
    path: &Path,
    edition: Edition,
    team: &str,
    version: Option<&str>,
) -> Result<(), &'static str> {
    let m = fs::symlink_metadata(path).map_err(|_| "root_app_missing")?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("root_app_type");
    }
    let args = app_signature_args(path, edition, team);
    run(
        "/usr/bin/codesign",
        &args.iter().map(|arg| arg.as_os_str()).collect::<Vec<_>>(),
    )?;
    if let Some(version) = version {
        let out = Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
            .arg(path.join("Contents/Info.plist"))
            .output()
            .map_err(|_| "root_app_version")?;
        if !out.status.success() || String::from_utf8_lossy(&out.stdout).trim() != version {
            return Err("root_app_version");
        }
    }
    Ok(())
}
fn compare_cdhash(output: &[u8], expected: &str) -> Result<(), &'static str> {
    if expected.len() != 40 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("root_app_cdhash");
    }
    let text = std::str::from_utf8(output).map_err(|_| "root_app_cdhash")?;
    let mut hashes = text.lines().filter_map(|line| line.strip_prefix("CDHash="));
    match (hashes.next(), hashes.next()) {
        (Some(hash), None) if hash.eq_ignore_ascii_case(expected) => Ok(()),
        _ => Err("root_app_cdhash"),
    }
}
fn verify_cdhash(path: &Path, expected: &str) -> Result<(), &'static str> {
    let output = Command::new("/usr/bin/codesign")
        .args(["--display", "--verbose=4"])
        .arg(path)
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .map_err(|_| "root_app_cdhash")?;
    if !output.status.success() || output.stderr.len() > 65536 {
        return Err("root_app_cdhash");
    }
    compare_cdhash(&output.stderr, expected)
}
struct PrivateStage(PathBuf);
impl PrivateStage {
    fn new() -> Result<Self, &'static str> {
        use std::ffi::{CStr, CString};
        let path = Path::new(SYSTEM_ROOT).join(".transaction-XXXXXX");
        let mut name = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| "root_stage_path")?
            .into_bytes_with_nul();
        let result = unsafe { libc::mkdtemp(name.as_mut_ptr().cast()) };
        if result.is_null() {
            return Err("root_stage_create");
        }
        let path = PathBuf::from(
            unsafe { CStr::from_ptr(result) }
                .to_str()
                .map_err(|_| "root_stage_path")?,
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "root_stage_mode")?;
        safe_root(&path)?;
        Ok(Self(path))
    }
}
impl Drop for PrivateStage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
/// Run only after Authorization Services granted an administrator operation.
/// Ordinary XPC callers cannot submit this enum or shell commands.
pub fn execute(op: RootOperation) -> Result<(), &'static str> {
    let _lock = RootLock::acquire()?;
    if Path::new(SYSTEM_ROOT).join("repair-journal.json").exists() {
        return Err("repair_recovery_required");
    }
    let team = match &op {
        RootOperation::InstallPackage { team, .. }
        | RootOperation::ReplaceApp { team, .. }
        | RootOperation::RestoreApp { team, .. } => team,
    };
    if !team_valid(team) {
        return Err("root_team");
    }
    let mut owner = fs::metadata("/dev/console")
        .map_err(|_| "root_console_owner")?
        .uid();
    if Path::new(INSTALL_RECORD).exists() {
        let record = crate::platform::load_root_record(Path::new(INSTALL_RECORD))?;
        owner = record.owner_uid;
        if record.developer_team()? != team {
            return Err("root_team_changed");
        }
    }
    match op {
        RootOperation::InstallPackage {
            source,
            team,
            window,
            sha256,
        } => {
            if !source.is_absolute() {
                return Err("root_source");
            }
            let stage = PrivateStage::new()?;
            let pkg = stage.0.join("components.pkg");
            let input = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&source)
                .map_err(|_| "root_package_source")?;
            if !input
                .metadata()
                .map_err(|_| "root_package_source")?
                .is_file()
            {
                return Err("root_package_type");
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&pkg)
                .map_err(|_| "root_package_stage")?;
            std::io::copy(
                &mut std::io::Read::take(input, 1024 * 1024 * 1024 + 1),
                &mut output,
            )
            .map_err(|_| "root_package_copy")?;
            if output.metadata().map_err(|_| "root_package_copy")?.len() > 1024 * 1024 * 1024 {
                return Err("root_package_size");
            }
            output.sync_all().map_err(|_| "root_package_sync")?;
            if let Some(expected) = sha256 {
                if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("root_package_hash");
                }
                let actual = Command::new("/usr/bin/shasum")
                    .args(["-a", "256"])
                    .arg(&pkg)
                    .output()
                    .map_err(|_| "root_package_hash")?;
                if !actual.status.success()
                    || String::from_utf8_lossy(&actual.stdout)
                        .split_whitespace()
                        .next()
                        != Some(expected.as_str())
                {
                    return Err("root_package_hash");
                }
            }
            let proof = Command::new("/usr/sbin/pkgutil")
                .arg("--check-signature")
                .arg(&pkg)
                .env("LC_ALL", "C")
                .output()
                .map_err(|_| "root_package_signature")?;
            if !proof.status.success()
                || !String::from_utf8_lossy(&proof.stdout).lines().any(|l| {
                    l.contains("Developer ID Installer:")
                        && l.trim().ends_with(&format!("({team})"))
                })
            {
                return Err("root_package_signature");
            }
            run(
                "/usr/sbin/spctl",
                &[
                    "--assess".as_ref(),
                    "--type".as_ref(),
                    "install".as_ref(),
                    pkg.as_os_str(),
                ],
            )?;
            if let Some(window) = &window {
                window.validate()?;
            }
            ensure_claude_stopped(owner)?;
            start_installer_barrier()?;
            let result = run(
                "/usr/sbin/installer",
                &[
                    "-pkg".as_ref(),
                    pkg.as_os_str(),
                    "-target".as_ref(),
                    "/".as_ref(),
                ],
            );
            // A returned installer client confirms its request completed, even on failure.
            // If this runner dies first, the signed package postinstall clears the barrier.
            let cleared = match fs::remove_file(barrier_path()) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err("root_installer_barrier_clear"),
            };
            result.and(cleared)
        }
        RootOperation::ReplaceApp {
            source,
            edition,
            team,
            old,
            new,
            window,
            cdhash,
        } => {
            if !version_valid(&old) || !version_valid(&new) || !source.is_absolute() {
                return Err("root_app_parameters");
            }
            let stage = PrivateStage::new()?;
            let incoming = stage.0.join("incoming.app");
            let dest = app_path(edition);
            let backup = backup_path(edition);
            if fs::metadata("/Applications")
                .map_err(|_| "applications")?
                .dev()
                != fs::metadata(&stage.0).map_err(|_| "root_stage")?.dev()
            {
                return Err("app_same_volume_required");
            }
            verify_app(&dest, edition, &team, Some(&old))?;
            let identity = fs::symlink_metadata(&dest).map_err(|_| "root_app_metadata")?;
            run(
                "/usr/bin/ditto",
                &[source.as_os_str(), incoming.as_os_str()],
            )?;
            run(
                "/usr/sbin/chown",
                &["-R".as_ref(), "root:wheel".as_ref(), incoming.as_os_str()],
            )?;
            run(
                "/bin/chmod",
                &["-R".as_ref(), "go-w".as_ref(), incoming.as_os_str()],
            )?;
            verify_app(&incoming, edition, &team, Some(&new))?;
            verify_cdhash(&incoming, &cdhash)?;
            run(
                "/usr/sbin/spctl",
                &[
                    "--assess".as_ref(),
                    "--type".as_ref(),
                    "execute".as_ref(),
                    incoming.as_os_str(),
                ],
            )?;
            window.validate()?;
            ensure_claude_stopped(owner)?;
            if backup.exists() {
                verify_app(&backup, edition, &team, None)?;
                fs::remove_dir_all(&backup).map_err(|_| "app_old_backup")?;
            }
            fs::rename(&dest, &backup).map_err(|_| "app_backup_rename")?;
            let moved = fs::symlink_metadata(&backup).map_err(|_| "app_backup_metadata")?;
            if moved.file_type().is_symlink()
                || (moved.dev(), moved.ino()) != (identity.dev(), identity.ino())
            {
                return Err("app_identity_changed");
            }
            fs::File::open(SYSTEM_ROOT)
                .and_then(|f| f.sync_all())
                .map_err(|_| "app_backup_sync")?;
            exclusive_rename(&incoming, &dest)?;
            File::open("/Applications")
                .and_then(|f| f.sync_all())
                .map_err(|_| "app_install_sync")
        }
        RootOperation::RestoreApp {
            edition,
            team,
            old,
            new,
        } => {
            if !version_valid(&old) || !version_valid(&new) {
                return Err("root_app_parameters");
            }
            let dest = app_path(edition);
            let backup = backup_path(edition);
            ensure_claude_stopped(owner)?;
            if verify_app(&dest, edition, &team, Some(&old)).is_ok() {
                return Ok(());
            }
            verify_app(&backup, edition, &team, Some(&old))?;
            if dest.exists() {
                verify_app(&dest, edition, &team, Some(&new))?;
                fs::remove_dir_all(&dest).map_err(|_| "app_recovery_remove")?;
            }
            exclusive_rename(&backup, &dest)?;
            File::open("/Applications")
                .and_then(|f| f.sync_all())
                .map_err(|_| "app_recovery_sync")
        }
    }
}
#[cfg(target_os = "macos")]
fn exclusive_rename(from: &Path, to: &Path) -> Result<(), &'static str> {
    use std::ffi::CString;
    let from = CString::new(from.as_os_str().as_encoded_bytes()).map_err(|_| "app_path")?;
    let to = CString::new(to.as_os_str().as_encoded_bytes()).map_err(|_| "app_path")?;
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } == 0 {
        Ok(())
    } else {
        Err("app_destination_changed")
    }
}
#[cfg(not(target_os = "macos"))]
fn exclusive_rename(_: &Path, _: &Path) -> Result<(), &'static str> {
    Err("macos_required")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source_local_fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("switcher-local-root-{name}-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn source_local_pending_survives_previous_boot_barrier_removal() {
        let root = source_local_fixture("cross-boot");
        let journal = root.join("source-local-transaction.json");
        let contents = br#"{"stage":"replacing","boot_id":"previous-boot"}"#;
        fs::write(&journal, contents).unwrap();
        let barrier = root.join("component-installer-active");
        fs::write(&barrier, b"previous-boot").unwrap();
        assert_eq!(
            check_source_local_transaction(&root),
            Err("source_local_recovery_required")
        );
        // Model the next boot removing its obsolete temporary installer barrier.
        fs::remove_file(&barrier).unwrap();
        assert_eq!(
            check_source_local_transaction(&root),
            Err("source_local_recovery_required")
        );
        assert_eq!(fs::read(&journal).unwrap(), contents);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn source_local_bad_journal_type_or_contents_never_bypass_gate() {
        use std::os::unix::fs::symlink;
        let root = source_local_fixture("bad-journal");
        let journal = root.join("source-local-transaction.json");
        for bytes in [b"invalid json".as_slice(), b"", b"{\"committed\":true}"] {
            fs::write(&journal, bytes).unwrap();
            assert_eq!(
                check_source_local_transaction(&root),
                Err("source_local_recovery_required")
            );
        }
        fs::remove_file(&journal).unwrap();
        fs::create_dir(&journal).unwrap();
        assert_eq!(
            check_source_local_transaction(&root),
            Err("source_local_recovery_required")
        );
        fs::remove_dir(&journal).unwrap();
        symlink(root.join("missing-journal-target"), &journal).unwrap();
        assert_eq!(
            check_source_local_transaction(&root),
            Err("source_local_recovery_required")
        );
        fs::remove_file(&journal).unwrap();
        let raw = std::ffi::CString::new(journal.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
        assert_eq!(
            check_source_local_transaction(&root),
            Err("source_local_recovery_required")
        );
        fs::remove_file(&journal).unwrap();
        let not_directory = root.join("invalid-parent");
        fs::write(&not_directory, b"file").unwrap();
        assert_eq!(
            check_source_local_transaction(&not_directory),
            Err("source_local_recovery_required")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn source_local_absent_journal_allows_normal_root_gate_to_continue() {
        let root = source_local_fixture("no-journal");
        assert_eq!(check_source_local_transaction(&root), Ok(()));
        assert!(!root.join("source-local-transaction.json").exists());
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn copied_app_must_match_sealed_cdhash_exactly_once() {
        let expected = "0123456789abcdef0123456789abcdef01234567";
        assert!(compare_cdhash(format!("CDHash={expected}\n").as_bytes(), expected).is_ok());
        assert_eq!(
            compare_cdhash(
                b"CDHash=ffffffffffffffffffffffffffffffffffffffff\n",
                expected
            ),
            Err("root_app_cdhash")
        );
        assert_eq!(
            compare_cdhash(
                format!("CDHash={expected}\nCDHash={expected}\n").as_bytes(),
                expected
            ),
            Err("root_app_cdhash")
        );
        assert_eq!(compare_cdhash(b"", expected), Err("root_app_cdhash"));
        assert_eq!(
            compare_cdhash(b"CDHash=bad\n", "bad"),
            Err("root_app_cdhash")
        );
    }
    #[test]
    fn inherited_transaction_lock_survives_runner_descriptor_close() {
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("switcher-root-lock-{}", std::process::id()));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        let lock = RootLock::lock(file).unwrap();
        assert_eq!(
            unsafe { libc::fcntl(lock.held_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        let mut child = Command::new("/bin/sleep").arg("0.3").spawn().unwrap();
        drop(lock);
        let competing = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert_eq!(
            RootLock::lock(competing).err(),
            Some("root_transaction_busy")
        );
        child.wait().unwrap();
        let after = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(RootLock::lock(after).is_ok());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn exclusive_rename_does_not_follow_or_overwrite_destination_symlink() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("switcher-root-rename-{}", std::process::id()));
        fs::create_dir(&base).unwrap();
        fs::create_dir(base.join("incoming")).unwrap();
        fs::write(base.join("incoming/proof"), b"new").unwrap();
        fs::create_dir(base.join("victim")).unwrap();
        symlink(base.join("victim"), base.join("dest")).unwrap();
        assert_eq!(
            exclusive_rename(&base.join("incoming"), &base.join("dest")),
            Err("app_destination_changed")
        );
        assert!(base.join("incoming/proof").is_file());
        assert!(!base.join("victim/proof").exists());
        assert!(
            fs::symlink_metadata(base.join("dest"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn root_gate_rechecks_expiry_clock_and_boot_after_authorization() {
        let window = RootWindow {
            issued: 100,
            expires: 200,
            utc: 150,
            continuous_ms: 1000,
            boot_id: "boot".into(),
        };
        assert!(window.validate_observation(199, 50_000, "boot").is_ok());
        assert_eq!(
            window.validate_observation(200, 51_000, "boot"),
            Err("metadata_expired")
        );
        assert_eq!(
            window.validate_observation(151, 2000, "other"),
            Err("clock_recovery_required")
        );
        assert_eq!(
            window.validate_observation(1000, 2000, "boot"),
            Err("clock_recovery_required")
        );
    }
    #[test]
    fn unprivileged_root_runner_fails_before_any_system_path_mutation() {
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        assert_eq!(
            execute(RootOperation::RestoreApp {
                edition: Edition::Public,
                team: "TESTTEAM01".into(),
                old: "1.0.0".into(),
                new: "1.1.0".into()
            }),
            Err("root_required")
        );
    }
}
