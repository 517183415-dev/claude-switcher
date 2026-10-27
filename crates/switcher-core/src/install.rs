//! Durable installation coordinator. Production mutation stays behind a signed PKG backend.
use crate::storage::{self, LifecycleLock};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use switcher_domain::{
    Edition,
    system::{GUARDIAN_LABEL, HELPER_LABEL, INSTALL_RECORD},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallIntent {
    Install,
    ReplaceEdition,
    Uninstall,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Preview,
    Authorized,
    BackupSaved,
    ComponentsStaged,
    HealthVerified,
    Committed,
    RollingBack,
    Recovered,
    RecoveryRequired,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallPreview {
    pub owner_uid: u32,
    pub intent: InstallIntent,
    pub previous_edition: Option<Edition>,
    pub edition: Edition,
    pub preserves_accounts: bool,
    pub services: Vec<String>,
    pub system_paths: Vec<String>,
    pub requires_administrator: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallJournal {
    pub schema_version: u32,
    pub preview: InstallPreview,
    pub stage: Stage,
    pub completed: Vec<String>,
    pub pending: Option<String>,
    pub errors: Vec<String>,
}
/// Implementations must validate Apple Installer signature and the pinned Team ID before staging.
/// Each method is idempotent: recovery can repeat an operation interrupted before its journal commit.
pub trait InstallBackend {
    fn authorize(&mut self) -> Result<(), &'static str>;
    fn backup(&mut self) -> Result<(), &'static str>;
    fn stage_signed_components(&mut self, preview: &InstallPreview) -> Result<(), &'static str>;
    fn verify_health(&mut self, preview: &InstallPreview) -> Result<(), &'static str>;
    fn commit_root_record(&mut self, preview: &InstallPreview) -> Result<(), &'static str>;
    fn is_committed(&self, _preview: &InstallPreview) -> Result<bool, &'static str> {
        Ok(false)
    }
    fn restore_backup(&mut self, preview: &InstallPreview) -> Result<(), &'static str>;
}
pub struct Installer {
    directory: PathBuf,
}
impl Installer {
    pub fn new(directory: PathBuf) -> Result<Self, &'static str> {
        storage::create_private_directory(&directory).map_err(|_| "install_directory")?;
        Ok(Self { directory })
    }
    fn journal_path(&self) -> PathBuf {
        self.directory.join("installation.json")
    }
    pub fn preview(
        &self,
        owner_uid: u32,
        edition: Edition,
        previous: Option<Edition>,
        intent: InstallIntent,
    ) -> Result<InstallPreview, &'static str> {
        if owner_uid == 0 || owner_uid != unsafe { libc::geteuid() } {
            return Err("install_owner_uid");
        }
        if previous.is_some_and(|old| old != edition) && intent != InstallIntent::ReplaceEdition {
            return Err("explicit_edition_replacement_required");
        }
        if intent == InstallIntent::ReplaceEdition
            && (previous.is_none() || previous == Some(edition))
        {
            return Err("replacement_requires_other_edition");
        }
        Ok(InstallPreview {
            owner_uid,
            intent,
            previous_edition: previous,
            edition,
            preserves_accounts: true,
            services: vec![HELPER_LABEL.into(), GUARDIAN_LABEL.into()],
            system_paths: vec![
                INSTALL_RECORD.into(),
                "/Library/PrivilegedHelperTools/local.claudeswitcher.helper".into(),
                "/Library/LaunchDaemons/local.claudeswitcher.helper.plist".into(),
            ],
            requires_administrator: true,
        })
    }
    fn save(&self, journal: &InstallJournal) -> Result<(), &'static str> {
        storage::atomic_json(&self.journal_path(), journal).map_err(|_| "install_journal_write")
    }
    pub fn inspect(&self) -> Result<InstallJournal, &'static str> {
        let journal: InstallJournal =
            storage::read_json(&self.journal_path()).map_err(|_| "install_journal_read")?;
        if journal.schema_version != 1 {
            return Err("install_journal_schema");
        }
        Ok(journal)
    }
    /// Cancel before authorization. An interrupted transaction must be recovered instead.
    pub fn cancel(&self, preview: InstallPreview) -> Result<InstallJournal, &'static str> {
        let _lock = LifecycleLock::try_acquire(&self.directory).map_err(|_| "lifecycle_busy")?;
        self.ensure_no_pending()?;
        let journal = InstallJournal {
            schema_version: 1,
            preview,
            stage: Stage::Cancelled,
            completed: vec![],
            pending: None,
            errors: vec![],
        };
        self.save(&journal)?;
        Ok(journal)
    }
    fn ensure_no_pending(&self) -> Result<(), &'static str> {
        if self.journal_path().exists() {
            let journal = self.inspect()?;
            if !matches!(
                journal.stage,
                Stage::Committed | Stage::Recovered | Stage::Cancelled
            ) {
                return Err("installation_recovery_required");
            }
        }
        Ok(())
    }
    pub fn execute(
        &self,
        preview: InstallPreview,
        backend: &mut impl InstallBackend,
    ) -> Result<InstallJournal, &'static str> {
        let _lock = LifecycleLock::try_acquire(&self.directory).map_err(|_| "lifecycle_busy")?;
        self.ensure_no_pending()?;
        // Rebuild the preview from fixed fields; never trust serialized lists of privileged paths.
        let expected = self.preview(
            preview.owner_uid,
            preview.edition,
            preview.previous_edition,
            preview.intent,
        )?;
        if serde_json::to_vec(&expected).ok() != serde_json::to_vec(&preview).ok() {
            return Err("install_preview_changed");
        }
        let mut journal = InstallJournal {
            schema_version: 1,
            preview,
            stage: Stage::Preview,
            completed: vec![],
            pending: Some("authorize".into()),
            errors: vec![],
        };
        self.save(&journal)?;
        if let Err(code) = backend.authorize() {
            journal.stage = Stage::Cancelled;
            journal.pending = None;
            journal.errors.push(code.into());
            self.save(&journal)?;
            return Ok(journal);
        }
        journal.stage = Stage::Authorized;
        self.save(&journal)?;
        for (operation, stage) in [
            ("backup", Stage::BackupSaved),
            ("stage", Stage::ComponentsStaged),
            ("health", Stage::HealthVerified),
            ("commit", Stage::Committed),
        ] {
            journal.pending = Some(operation.into());
            self.save(&journal)?;
            let result = match operation {
                "backup" => backend.backup(),
                "stage" => backend.stage_signed_components(&journal.preview),
                "health" => backend.verify_health(&journal.preview),
                "commit" => backend.commit_root_record(&journal.preview),
                _ => unreachable!(),
            };
            if let Err(code) = result {
                if operation == "commit" && backend.is_committed(&journal.preview)? {
                    journal.stage = Stage::Committed;
                    journal.pending = None;
                    self.save(&journal)?;
                    return Ok(journal);
                }
                journal.errors.push(code.into());
                journal.stage = Stage::RollingBack;
                self.save(&journal)?;
                return self.rollback(journal, backend);
            }
            journal.completed.push(operation.into());
            journal.pending = None;
            journal.stage = stage;
            self.save(&journal)?;
        }
        Ok(journal)
    }
    fn rollback(
        &self,
        mut journal: InstallJournal,
        backend: &mut impl InstallBackend,
    ) -> Result<InstallJournal, &'static str> {
        // Even a partially completed backup must support idempotent restore.
        journal.pending = Some("restore".into());
        self.save(&journal)?;
        match backend.restore_backup(&journal.preview) {
            Ok(()) => {
                journal.stage = Stage::Recovered;
                journal.pending = None;
            }
            Err(code) => {
                journal.stage = Stage::RecoveryRequired;
                journal.errors.push(code.into());
            }
        }
        self.save(&journal)?;
        Ok(journal)
    }
    pub fn recover(
        &self,
        backend: &mut impl InstallBackend,
    ) -> Result<InstallJournal, &'static str> {
        let _lock = LifecycleLock::try_acquire(&self.directory).map_err(|_| "lifecycle_busy")?;
        let mut journal = self.inspect()?;
        if journal.pending.as_deref() == Some("commit") && backend.is_committed(&journal.preview)? {
            journal.stage = Stage::Committed;
            journal.pending = None;
            self.save(&journal)?;
            return Ok(journal);
        }
        if matches!(
            journal.stage,
            Stage::Committed | Stage::Recovered | Stage::Cancelled
        ) {
            return Ok(journal);
        }
        self.rollback(journal, backend)
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    #[derive(Default)]
    struct Fake {
        fail: Option<&'static str>,
        committed: bool,
        calls: Vec<&'static str>,
    }
    impl Fake {
        fn step(&mut self, s: &'static str) -> Result<(), &'static str> {
            self.calls.push(s);
            if self.fail == Some(s) {
                Err("synthetic_fault")
            } else {
                Ok(())
            }
        }
    }
    impl InstallBackend for Fake {
        fn authorize(&mut self) -> Result<(), &'static str> {
            self.step("authorize")
        }
        fn backup(&mut self) -> Result<(), &'static str> {
            self.step("backup")
        }
        fn stage_signed_components(&mut self, _: &InstallPreview) -> Result<(), &'static str> {
            self.step("stage")
        }
        fn verify_health(&mut self, _: &InstallPreview) -> Result<(), &'static str> {
            self.step("health")
        }
        fn commit_root_record(&mut self, _: &InstallPreview) -> Result<(), &'static str> {
            self.step("commit")
        }
        fn is_committed(&self, _: &InstallPreview) -> Result<bool, &'static str> {
            Ok(self.committed)
        }
        fn restore_backup(&mut self, _preview: &InstallPreview) -> Result<(), &'static str> {
            self.step("restore")
        }
    }
    pub(super) fn fixture() -> (Installer, InstallPreview) {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-install-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fixture_at(path).unwrap()
    }
    fn fixture_at(path: PathBuf) -> std::io::Result<(Installer, InstallPreview)> {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        let result = (|| {
            let installer = Installer::new(path.clone()).map_err(std::io::Error::other)?;
            let preview = installer
                .preview(
                    unsafe { libc::geteuid() },
                    Edition::Personal,
                    None,
                    InstallIntent::Install,
                )
                .map_err(std::io::Error::other)?;
            Ok((installer, preview))
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&path);
        }
        result
    }
    #[test]
    fn fixture_collision_preserves_existing_directory() {
        use std::os::unix::fs::DirBuilderExt;
        let parent = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-install-collision-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&parent)
            .unwrap();
        let collision = parent.join("existing");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&collision)
            .unwrap();
        let sentinel = collision.join("sentinel");
        std::fs::write(&sentinel, b"preserve").unwrap();
        let result = fixture_at(collision);
        let contents = std::fs::read(&sentinel).unwrap();
        std::fs::remove_dir_all(parent).unwrap();
        assert_eq!(contents, b"preserve");
        assert!(
            matches!(result, Err(ref error) if error.kind() == std::io::ErrorKind::AlreadyExists)
        );
    }
    #[test]
    fn cancelled_authorization_preserves_old_components() {
        let (i, p) = fixture();
        let mut b = Fake {
            fail: Some("authorize"),
            ..Default::default()
        };
        assert_eq!(i.execute(p, &mut b).unwrap().stage, Stage::Cancelled);
        assert_eq!(b.calls, vec!["authorize"]);
        std::fs::remove_dir_all(i.directory()).unwrap();
    }
    #[test]
    fn every_mutation_failure_restores_and_persists() {
        for fail in ["backup", "stage", "health", "commit"] {
            let (i, p) = fixture();
            let mut b = Fake {
                fail: Some(fail),
                ..Default::default()
            };
            assert_eq!(i.execute(p, &mut b).unwrap().stage, Stage::Recovered);
            assert_eq!(i.inspect().unwrap().stage, Stage::Recovered);
            assert_eq!(b.calls.last(), Some(&"restore"));
            std::fs::remove_dir_all(i.directory()).unwrap();
        }
    }
    #[test]
    fn durable_root_commit_prevents_rollback_after_lost_reply_or_restart() {
        let (i, p) = fixture();
        let mut b = Fake {
            fail: Some("commit"),
            committed: true,
            ..Default::default()
        };
        assert_eq!(
            i.execute(p.clone(), &mut b).unwrap().stage,
            Stage::Committed
        );
        assert!(!b.calls.contains(&"restore"));
        i.save(&InstallJournal {
            schema_version: 1,
            preview: p,
            stage: Stage::HealthVerified,
            completed: vec!["health".into()],
            pending: Some("commit".into()),
            errors: vec![],
        })
        .unwrap();
        b.calls.clear();
        assert_eq!(i.recover(&mut b).unwrap().stage, Stage::Committed);
        assert!(b.calls.is_empty());
        std::fs::remove_dir_all(i.directory()).unwrap();
    }
    #[test]
    fn recovery_failure_blocks_new_install() {
        let (i, p) = fixture();
        let mut journal = InstallJournal {
            schema_version: 1,
            preview: p.clone(),
            stage: Stage::ComponentsStaged,
            completed: vec!["backup".into()],
            pending: Some("stage".into()),
            errors: vec![],
        };
        i.save(&journal).unwrap();
        assert_eq!(
            i.execute(p.clone(), &mut Fake::default()).unwrap_err(),
            "installation_recovery_required"
        );
        let mut b = Fake {
            fail: Some("restore"),
            ..Default::default()
        };
        journal = i.recover(&mut b).unwrap();
        assert_eq!(journal.stage, Stage::RecoveryRequired);
        assert_eq!(
            i.recover(&mut Fake::default()).unwrap().stage,
            Stage::Recovered
        );
        assert_eq!(
            i.execute(p, &mut Fake::default()).unwrap().stage,
            Stage::Committed
        );
        std::fs::remove_dir_all(i.directory()).unwrap();
    }
    #[test]
    fn edition_replacement_is_explicit_and_paths_fixed() {
        let (i, mut p) = fixture();
        assert!(
            i.preview(
                p.owner_uid,
                Edition::Public,
                Some(Edition::Personal),
                InstallIntent::Install
            )
            .is_err()
        );
        p.system_paths.push("/tmp/arbitrary".into());
        assert_eq!(
            i.execute(p, &mut Fake::default()).unwrap_err(),
            "install_preview_changed"
        );
        std::fs::remove_dir_all(i.directory()).unwrap();
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn validate_installed_identity(
    edition: Edition,
    uid: u32,
) -> Result<switcher_domain::system::InstallRecord, &'static str> {
    use std::os::unix::fs::MetadataExt;
    let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
    if record.edition != edition || record.owner_uid != uid || uid != unsafe { libc::geteuid() } {
        return Err("installed_owner_or_edition");
    }
    for path in [
        "/Library/PrivilegedHelperTools/local.claudeswitcher.helper",
        "/Library/LaunchDaemons/local.claudeswitcher.helper.plist",
    ] {
        let metadata =
            std::fs::symlink_metadata(path).map_err(|_| "installed_component_missing")?;
        if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err("installed_component_permissions");
        }
    }
    verify_code_requirement(
        Path::new("/Library/PrivilegedHelperTools/local.claudeswitcher.helper"),
        &record.helper_requirement()?,
        Path::new("/usr/bin/codesign"),
    )?;
    Ok(record)
}
/// Installation identity is required even while a repair journal blocks guardian health.
#[cfg(target_os = "macos")]
pub fn validate_installed(
    edition: Edition,
    uid: u32,
) -> Result<switcher_domain::system::InstallRecord, &'static str> {
    let record = validate_installed_identity(edition, uid)?;
    validate_installed_health(uid)?;
    Ok(record)
}
#[cfg(target_os = "macos")]
pub(crate) fn validate_installed_health(uid: u32) -> Result<(), &'static str> {
    let home = crate::config::home_dir()?;
    crate::guardian::read_health(
        &home.join(".claude-switcher/v2/guardian-health.json"),
        uid,
        crate::checks::unix_now(),
    )?;
    Ok(())
}
#[cfg(target_os = "macos")]
static REPAIR_CLIENT: std::sync::Mutex<Option<switcher_helper::xpc::NativeClient>> =
    std::sync::Mutex::new(None);
#[cfg(target_os = "macos")]
pub fn preview_repair(
    target: switcher_domain::system::RegionPolicy,
) -> Result<switcher_domain::system::HelperResponse, &'static str> {
    use switcher_domain::system::*;
    let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
    let mut connection = switcher_helper::xpc::NativeClient::connect(&record, ClientRole::Gui)?;
    let result = connection.call(HelperAction::PreviewRepair { target })?;
    if result.ok {
        *REPAIR_CLIENT.lock().map_err(|_| "repair_busy")? = Some(connection);
    }
    Ok(result)
}
#[cfg(target_os = "macos")]
pub fn execute_repair(
    plan_id: String,
) -> Result<switcher_domain::system::HelperResponse, &'static str> {
    let home = crate::config::home_dir()?;
    let lifecycle = LifecycleLock::try_acquire(&home.join(".claude-switcher/v2"))
        .map_err(|_| "lifecycle_busy")?;
    authorize_repair_with_lock(&plan_id, &lifecycle)?;
    execute_repair_with_lock(plan_id, &lifecycle)
}
/// Obtain and independently verify this operation's administrator right before user writes.
#[cfg(target_os = "macos")]
pub fn authorize_repair_with_lock(plan_id: &str, lock: &LifecycleLock) -> Result<(), &'static str> {
    let home = crate::config::home_dir()?;
    repair_with_lock_at(&home.join(".claude-switcher/v2"), lock, || {
        let mut guard = REPAIR_CLIENT.lock().map_err(|_| "repair_busy")?;
        let client = guard.as_mut().ok_or("repair_preview_required")?;
        let authorization = switcher_helper::native_authorization::Authorization::acquire()?;
        let response = client.call(switcher_domain::system::HelperAction::AuthorizeRepair {
            plan_id: plan_id.into(),
            recovery: false,
            external_form: authorization.external_form()?,
        })?;
        if response.ok && response.code == "repair_authorized" {
            Ok(())
        } else {
            Err("repair_authorization_rejected")
        }
    })
}
#[cfg(not(target_os = "macos"))]
pub fn authorize_repair_with_lock(_: &str, _: &LifecycleLock) -> Result<(), &'static str> {
    Err("macos_required")
}
/// Reuses the region transaction's existing lock; never reacquires its OS lock.
#[cfg(target_os = "macos")]
pub fn execute_repair_with_lock(
    plan_id: String,
    lock: &LifecycleLock,
) -> Result<switcher_domain::system::HelperResponse, &'static str> {
    let home = crate::config::home_dir()?;
    repair_with_lock_at(&home.join(".claude-switcher/v2"), lock, || {
        let mut guard = REPAIR_CLIENT.lock().map_err(|_| "repair_busy")?;
        let mut client = guard.take().ok_or("repair_preview_required")?;
        client.call(switcher_domain::system::HelperAction::ApplyRepair { plan_id })
    })
}
/// Reads the root-protected result of one plan on a new authenticated connection.
/// Busy, timeout, missing or mismatched receipt means the caller must preserve recovery state.
#[cfg(target_os = "macos")]
pub fn repair_status(
    plan_id: String,
) -> Result<switcher_domain::system::HelperResponse, &'static str> {
    use switcher_domain::system::*;
    let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
    let mut client = switcher_helper::xpc::NativeClient::connect(&record, ClientRole::Gui)?;
    client.call(HelperAction::RepairStatus { plan_id })
}
/// Explicitly restores only the root journal belonging to this plan. Never supplies a target.
#[cfg(target_os = "macos")]
pub fn recover_repair(
    plan_id: String,
) -> Result<switcher_domain::system::HelperResponse, &'static str> {
    let authorization = switcher_helper::native_authorization::Authorization::acquire()?;
    recover_repair_authorized(plan_id, &authorization)
}
#[cfg(target_os = "macos")]
pub fn recover_repair_authorized(
    plan_id: String,
    authorization: &switcher_helper::native_authorization::Authorization,
) -> Result<switcher_domain::system::HelperResponse, &'static str> {
    use switcher_domain::system::*;
    let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
    let mut client = switcher_helper::xpc::NativeClient::connect(&record, ClientRole::Gui)?;
    let authorized = client.call(HelperAction::AuthorizeRepair {
        plan_id: plan_id.clone(),
        recovery: true,
        external_form: authorization.external_form()?,
    })?;
    if !authorized.ok {
        return Err("repair_authorization_rejected");
    }
    client.call(HelperAction::RecoverRepair { plan_id })
}
fn repair_with_lock_at<T>(
    root: &Path,
    lock: &LifecycleLock,
    send: impl FnOnce() -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    if !lock.protects(root) {
        return Err("lifecycle_lock_required");
    }
    send()
}
fn verify_binary_signature(path: &Path, team: &str, bundle: &str) -> Result<(), &'static str> {
    verify_binary_signature_with(path, team, bundle, Path::new("/usr/bin/codesign"))
}
fn verify_binary_signature_with(
    path: &Path,
    team: &str,
    bundle: &str,
    codesign: &Path,
) -> Result<(), &'static str> {
    let requirement = format!(
        "=anchor apple generic and certificate leaf[subject.OU] = \"{team}\" and identifier \"{bundle}\""
    );
    verify_code_requirement(path, &requirement[1..], codesign)
}
fn verify_code_requirement(
    path: &Path,
    requirement: &str,
    codesign: &Path,
) -> Result<(), &'static str> {
    let requirement = format!("={requirement}");
    let result = std::process::Command::new(codesign)
        .args(["--verify", "--strict", "--test-requirement", &requirement])
        .arg(path)
        .output()
        .map_err(|_| "component_signature_unavailable")?;
    if result.status.success() {
        Ok(())
    } else {
        Err("component_signature")
    }
}

/// Production backend. Both forward and recovery PKGs must be signed by the pinned team.
/// No mutation occurs during construction or package verification.
pub struct SignedPkgBackend {
    package: PathBuf,
    recovery_package: PathBuf,
    team: String,
    state: PathBuf,
    authorization: usize,
    window: Option<switcher_helper::root_operation::RootWindow>,
    expected_hash: Option<String>,
    previous: Option<Edition>,
    target: Option<Edition>,
    uid: u32,
}
impl SignedPkgBackend {
    pub fn new(
        package: PathBuf,
        recovery_package: PathBuf,
        team: String,
        state: PathBuf,
    ) -> Result<Self, &'static str> {
        if team.len() != 10
            || !team
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err("installer_team");
        }
        if !package.is_absolute() || !recovery_package.is_absolute() || package == recovery_package
        {
            return Err("installer_package_path");
        }
        storage::create_private_directory(&state).map_err(|_| "installer_state")?;
        Ok(Self {
            package,
            recovery_package,
            team,
            state,
            authorization: 0,
            window: None,
            expected_hash: None,
            previous: None,
            target: None,
            uid: unsafe { libc::geteuid() },
        })
    }
    pub fn verify_package(path: &Path, team: &str) -> Result<(), &'static str> {
        let meta = std::fs::symlink_metadata(path).map_err(|_| "package_missing")?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("package_type");
        }
        let output = std::process::Command::new("/usr/sbin/pkgutil")
            .arg("--check-signature")
            .arg(path)
            .env("LC_ALL", "C")
            .output()
            .map_err(|_| "package_signature_unavailable")?;
        let text = String::from_utf8_lossy(&output.stdout);
        if !output.status.success()
            || !text.lines().any(|s| {
                s.contains("Developer ID Installer:") && s.trim().ends_with(&format!("({team})"))
            })
        {
            return Err("package_signature");
        }
        let status = std::process::Command::new("/usr/sbin/spctl")
            .args(["--assess", "--type", "install"])
            .arg(path)
            .status()
            .map_err(|_| "package_assessment_unavailable")?;
        if status.success() {
            Ok(())
        } else {
            Err("package_notarization")
        }
    }
    fn install_package(&self, path: &Path) -> Result<(), &'static str> {
        Self::verify_package(path, &self.team)?;
        run_root_operation(
            &switcher_helper::root_operation::RootOperation::InstallPackage {
                source: path.to_owned(),
                team: self.team.clone(),
                window: self.window.clone(),
                sha256: self.expected_hash.clone(),
            },
            &self.team,
        )
    }
}
#[cfg(target_os = "macos")]
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn AuthorizationCreate(
        rights: *const std::ffi::c_void,
        environment: *const std::ffi::c_void,
        flags: u32,
        authorization: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn AuthorizationFree(authorization: *mut std::ffi::c_void, flags: u32) -> i32;
}
impl InstallBackend for SignedPkgBackend {
    fn authorize(&mut self) -> Result<(), &'static str> {
        Self::verify_package(&self.package, &self.team)?;
        Self::verify_package(&self.recovery_package, &self.team)?;
        #[cfg(target_os = "macos")]
        {
            #[repr(C)]
            struct Item {
                name: *const i8,
                len: usize,
                value: *mut std::ffi::c_void,
                flags: u32,
            }
            #[repr(C)]
            struct Rights {
                count: u32,
                items: *mut Item,
            }
            let mut item = Item {
                name: c"system.install.software".as_ptr(),
                len: 0,
                value: std::ptr::null_mut(),
                flags: 0,
            };
            let rights = Rights {
                count: 1,
                items: &mut item,
            };
            let mut auth = std::ptr::null_mut();
            let status = unsafe {
                AuthorizationCreate(
                    (&rights as *const Rights).cast(),
                    std::ptr::null(),
                    1 | 2 | 16,
                    &mut auth,
                )
            };
            if status != 0 || auth.is_null() {
                return Err("authorization_cancelled_or_denied");
            }
            self.authorization = auth as usize;
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("macos_required")
        }
    }
    fn backup(&mut self) -> Result<(), &'static str> {
        let home = crate::config::home_dir()?;
        let config = home.join(".claude-switcher/v2/config.json");
        if config.exists() {
            let value: serde_json::Value =
                storage::read_json(&config).map_err(|_| "configuration_backup_read")?;
            storage::atomic_json(&self.state.join("configuration-backup.json"), &value)
                .map_err(|_| "configuration_backup_write")?;
        }
        Ok(())
    }
    fn stage_signed_components(&mut self, p: &InstallPreview) -> Result<(), &'static str> {
        if self.authorization == 0 {
            return Err("authorization_required");
        }
        self.previous = p.previous_edition;
        self.target = Some(p.edition);
        self.uid = p.owner_uid;
        #[cfg(target_os = "macos")]
        if p.intent == InstallIntent::Uninstall {
            let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
            let mut client = switcher_helper::xpc::NativeClient::connect(
                &record,
                switcher_domain::system::ClientRole::Gui,
            )?;
            if !client
                .call(switcher_domain::system::HelperAction::RestoreManagedSystem)?
                .ok
            {
                return Err("uninstall_system_restore_conflict");
            }
        }
        self.install_package(&self.package)
    }
    fn verify_health(&mut self, p: &InstallPreview) -> Result<(), &'static str> {
        #[cfg(target_os = "macos")]
        {
            if p.intent == InstallIntent::Uninstall {
                if Path::new(INSTALL_RECORD).exists() {
                    Err("uninstall_components_remain")
                } else {
                    Ok(())
                }
            } else {
                validate_installed(p.edition, p.owner_uid).map(|_| ())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = p;
            Err("macos_required")
        }
    }
    fn commit_root_record(&mut self, p: &InstallPreview) -> Result<(), &'static str> {
        self.verify_health(p)?;
        #[cfg(target_os = "macos")]
        {
            if p.intent == InstallIntent::Uninstall {
                return Ok(());
            }
            let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
            let mut client = switcher_helper::xpc::NativeClient::connect(
                &record,
                switcher_domain::system::ClientRole::Gui,
            )?;
            let result = client.call(switcher_domain::system::HelperAction::CommitInstallation)?;
            if result.ok {
                Ok(())
            } else {
                Err("installation_commit_rejected")
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("macos_required")
        }
    }
    fn is_committed(&self, preview: &InstallPreview) -> Result<bool, &'static str> {
        #[cfg(target_os = "macos")]
        {
            let path =
                Path::new(switcher_domain::system::SYSTEM_ROOT).join("component-committed.json");
            if !path.exists() {
                return Ok(false);
            }
            let marker = switcher_helper::platform::load_root_record(&path)?;
            let current = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
            Ok(marker.owner_uid == preview.owner_uid
                && marker.edition == preview.edition
                && marker.developer_team()? == self.team
                && marker == current)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = preview;
            Err("macos_required")
        }
    }
    fn restore_backup(&mut self, preview: &InstallPreview) -> Result<(), &'static str> {
        self.previous = preview.previous_edition;
        self.uid = preview.owner_uid;
        self.install_package(&self.recovery_package)?;
        let backup = self.state.join("configuration-backup.json");
        if backup.exists() {
            let value: serde_json::Value =
                storage::read_json(&backup).map_err(|_| "configuration_restore_read")?;
            let home = crate::config::home_dir()?;
            storage::atomic_json(&home.join(".claude-switcher/v2/config.json"), &value)
                .map_err(|_| "configuration_restore_write")?;
        }
        #[cfg(target_os = "macos")]
        {
            if let Some(previous) = self.previous {
                validate_installed(previous, self.uid)?;
            } else if Path::new(INSTALL_RECORD).exists() {
                return Err("recovery_components_remain");
            }
        }
        Ok(())
    }
}
impl Drop for SignedPkgBackend {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if self.authorization != 0 {
            unsafe {
                AuthorizationFree(self.authorization as *mut std::ffi::c_void, 0);
            }
        }
    }
}

fn update_window(
    metadata: &crate::updates::Metadata,
) -> Result<switcher_helper::root_operation::RootWindow, &'static str> {
    let sample = crate::time::sample()?;
    Ok(switcher_helper::root_operation::RootWindow {
        issued: metadata.issued_at,
        expires: metadata.expires_at,
        utc: sample.utc_secs,
        continuous_ms: sample.continuous_ms,
        boot_id: sample.boot_id,
    })
}
fn validate_update_time(
    clock: &crate::time::ClockState,
    issued: u64,
    expires: u64,
    now: u64,
) -> Result<(), &'static str> {
    if !clock.trusted() {
        return Err("clock_recovery_required");
    }
    if now < issued || now >= expires {
        return Err("metadata_expired");
    }
    Ok(())
}
fn read_update_transaction(path: &Path) -> std::io::Result<UpdateTransaction> {
    let limit = crate::updates::STATE_LIMIT + 16384;
    let bytes = storage::read_private_bytes(path, limit)?;
    crate::signed::strict_json(&bytes, limit).map_err(std::io::Error::other)
}
#[cfg(any(target_os = "macos", test))]
fn ensure_account_idle(root: &Path) -> Result<(), &'static str> {
    if crate::accounts::account_recovery_pending_v2(root).map_err(|_| "account_journal")? {
        return Err("account_recovery_required");
    }
    Ok(())
}
#[cfg(any(target_os = "macos", test))]
fn ensure_update_transaction_idle(path: &Path) -> Result<(), &'static str> {
    match read_update_transaction(path) {
        Ok(previous) if !previous.committed => Err("update_recovery_required"),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("update_transaction_read"),
    }
}
#[cfg(target_os = "macos")]
fn run_root_operation(
    op: &switcher_helper::root_operation::RootOperation,
    team: &str,
) -> Result<(), &'static str> {
    let installed = Path::new("/Library/PrivilegedHelperTools/local.claudeswitcher.helper");
    let binary = if installed.exists() {
        installed.to_owned()
    } else {
        std::env::current_exe()
            .map_err(|_| "wait_signed_bootstrap")?
            .parent()
            .and_then(|p| p.parent())
            .ok_or("wait_signed_bootstrap")?
            .join("Resources/components/switcher-helper")
    };
    if !binary.is_file() {
        return Err("wait_signed_bootstrap");
    }
    verify_binary_signature(&binary, team, switcher_domain::system::HELPER_LABEL)?;
    let body = serde_json::to_string(op).map_err(|_| "root_operation_encoding")?;
    if body.len() > 16384 {
        return Err("root_operation_size");
    }
    run_administrator(&root_operation_script(&binary, &body, team)?)
}
#[cfg(target_os = "macos")]
fn root_operation_script(binary: &Path, body: &str, team: &str) -> Result<String, &'static str> {
    let requirement = format!(
        "=anchor apple generic and certificate leaf[subject.OU] = \"{team}\" and identifier \"local.claudeswitcher.helper\""
    );
    let script = format!(
        r#"set -eu
base='/Library/Application Support/claude-switcher'
for p in '/Library' '/Library/Application Support' "$base"; do
[ ! -L "$p" ] || exit 72
if [ -e "$p" ]; then
[ "$(/usr/bin/stat -f %u "$p")" = 0 ] || exit 73
/usr/bin/find "$p" -maxdepth 0 -perm +022 | /usr/bin/grep -q . && exit 74
fi
done
/bin/mkdir -p "$base"
/usr/sbin/chown root:wheel "$base"
/bin/chmod 755 "$base"
stage=$(/usr/bin/mktemp -d "$base/.bootstrap-XXXXXX")
/bin/chmod 700 "$stage"
trap '/bin/rm -rf "$stage"' EXIT
/bin/cp {} "$stage/runner"
/usr/sbin/chown root:wheel "$stage/runner"
/bin/chmod 700 "$stage/runner"
/usr/bin/codesign --verify --strict --test-requirement {} "$stage/runner"
"$stage/runner" --root-operation {}
"#,
        shell_literal(binary.to_str().ok_or("bootstrap_path_encoding")?),
        shell_literal(&requirement),
        shell_literal(body)
    );
    Ok(script)
}
/// A lock capability is retained for the complete native update transaction.
/// The desktop adapter persists its aggregate state; this journal survives its process dying.
#[cfg(target_os = "macos")]
pub struct NativeUpdateCoordinator<'a> {
    root: PathBuf,
    lock: &'a LifecycleLock,
    clock: crate::time::ClockState,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateTransaction {
    schema: u32,
    edition: Edition,
    team: String,
    old_app: String,
    old_helper: String,
    app: String,
    helper_changed: bool,
    helper_pending: bool,
    app_pending: bool,
    committed: bool,
    updates: crate::updates::UpdateState,
}
#[cfg(target_os = "macos")]
impl<'a> NativeUpdateCoordinator<'a> {
    pub fn new(
        root: &Path,
        lock: &'a LifecycleLock,
        clock: crate::time::ClockState,
    ) -> Result<Self, &'static str> {
        if !lock.protects(root) {
            return Err("lifecycle_lock_required");
        }
        Ok(Self {
            root: root.to_owned(),
            lock,
            clock,
        })
    }
    fn path(&self) -> PathBuf {
        self.root.join("update-install-transaction.json")
    }
    fn save(&self, journal: &UpdateTransaction) -> Result<(), &'static str> {
        if !self.lock.protects(&self.root) {
            return Err("lifecycle_lock_required");
        }
        let bytes = serde_json::to_vec(journal).map_err(|_| "update_transaction_encoding")?;
        if bytes.len() > crate::updates::STATE_LIMIT + 16384 {
            return Err("update_transaction_size");
        }
        storage::atomic_write(&self.path(), &bytes).map_err(|_| "update_transaction_write")
    }
    /// Real recovery keeps RecoveryRequired until both old signed components verify.
    /// Call under the same lock before accepting another candidate.
    pub fn recover(&mut self, state: &mut crate::updates::UpdateState) -> Result<(), &'static str> {
        let mut journal: UpdateTransaction = match read_update_transaction(&self.path()) {
            Ok(j) => j,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err("update_transaction_read"),
        };
        if journal.schema != 1 {
            return Err("update_transaction_schema");
        }
        if journal.committed {
            if state.candidate.as_ref().map(|c| &c.payload_hash)
                == journal.updates.candidate.as_ref().map(|c| &c.payload_hash)
            {
                *state = journal.updates;
            }
            return Ok(());
        }
        let uid = unsafe { libc::geteuid() };
        crate::accounts::ensure_claude_stopped(uid).map_err(|_| "claude_running")?;
        let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
        if record.owner_uid != uid
            || record.edition != journal.edition
            || record.developer_team()? != journal.team
        {
            return Err("update_recovery_identity");
        }
        ensure_account_idle(&self.root)?;
        journal.updates.recover_interrupted();
        self.save(&journal)?;
        if journal.helper_pending
            && (record.helper_version != journal.old_helper
                || Path::new(switcher_domain::system::SYSTEM_ROOT)
                    .join("component-pending")
                    .exists())
        {
            let old = switcher_helper::platform::load_root_record(
                &Path::new(switcher_domain::system::SYSTEM_ROOT)
                    .join("component-backup/install.json"),
            )?;
            if old.helper_version != journal.old_helper
                || old.edition != journal.edition
                || old.developer_team()? != journal.team
                || old.owner_uid != uid
            {
                return Err("update_recovery_snapshot");
            }
            let recovery = self.recovery_pkg(journal.edition);
            let backend = SignedPkgBackend::new(
                self.root.join("unused-forward.pkg"),
                recovery.clone(),
                journal.team.clone(),
                self.root.clone(),
            )?;
            backend.install_package(&recovery)?;
        }
        if journal.app_pending {
            run_root_operation(
                &switcher_helper::root_operation::RootOperation::RestoreApp {
                    edition: journal.edition,
                    team: journal.team.clone(),
                    old: journal.old_app.clone(),
                    new: journal.app.clone(),
                },
                &journal.team,
            )?;
        }
        let restored = wait_installed(journal.edition, uid)?;
        if restored.helper_version != journal.old_helper {
            return Err("update_recovery_helper_version");
        }
        verify_app_version(
            &installed_app(journal.edition),
            &journal.team,
            record.bundle_id(switcher_domain::system::ClientRole::Gui),
            &journal.old_app,
        )?;
        if journal
            .updates
            .candidate
            .as_ref()
            .is_some_and(|c| c.stage == crate::updates::InstallStage::RecoveryRequired)
        {
            journal.updates.advance_install(
                crate::updates::InstallStage::RecoveryRequired,
                crate::updates::InstallStage::RolledBack,
            )?;
        }
        *state = journal.updates.clone();
        journal.committed = true;
        self.save(&journal)
    }
    fn fresh_gate(
        &mut self,
        metadata: &crate::updates::Metadata,
        uid: u32,
    ) -> Result<u64, &'static str> {
        if !self.lock.protects(&self.root) {
            return Err("lifecycle_lock_required");
        }
        let sample = crate::time::sample()?;
        self.clock.observe(sample.clone());
        let now: u64 = sample.utc_secs.try_into().map_err(|_| "update_clock")?;
        validate_update_time(&self.clock, metadata.issued_at, metadata.expires_at, now)?;
        crate::accounts::ensure_claude_stopped(uid).map_err(|_| "claude_running")?;
        ensure_account_idle(&self.root)?;
        Ok(now)
    }
    fn recovery_pkg(&self, edition: Edition) -> PathBuf {
        self.root
            .join(match edition {
                Edition::Personal => "personal",
                Edition::Public => "public",
            })
            .join("update-recovery/helper.pkg")
    }
}
#[cfg(target_os = "macos")]
impl crate::updates::UpdateInstallCoordinator for NativeUpdateCoordinator<'_> {
    fn install(
        &mut self,
        prepared: &crate::updates::PreparedInstallation,
        state: &mut crate::updates::UpdateState,
    ) -> Result<(), &'static str> {
        use crate::updates::{InstallOrder, InstallReadiness, InstallStage as UStage};
        if !self.lock.protects(&self.root) {
            return Err("lifecycle_lock_required");
        }
        ensure_update_transaction_idle(&self.path())?;
        let sample = crate::time::sample()?;
        self.clock.observe(sample.clone());
        if !self.clock.trusted() {
            return Err("clock_recovery_required");
        }
        let uid = unsafe { libc::geteuid() };
        let metadata = prepared.candidate().metadata();
        let installed = validate_installed_identity(metadata.edition, uid)?;
        let installed_team = installed.developer_team()?.to_owned();
        validate_installed_health(uid)?;
        ensure_account_idle(&self.root)?;
        crate::accounts::ensure_claude_stopped(uid).map_err(|_| "claude_running")?;
        let app = installed_app(metadata.edition);
        verify_binary_signature(
            &app,
            &installed_team,
            installed.bundle_id(switcher_domain::system::ClientRole::Gui),
        )?;
        let old_app = app_version(&app)?;
        let _app_version = semver::Version::parse(&old_app).map_err(|_| "installed_app_version")?;
        let helper_version = semver::Version::parse(&installed.helper_version)
            .map_err(|_| "installed_helper_version")?;
        let final_helper = metadata
            .helper
            .as_ref()
            .map(|h| &h.version)
            .unwrap_or(&helper_version);
        if metadata.app.team_id != installed_team
            || !metadata.app.compatible_with.matches(final_helper)
            || metadata.helper.as_ref().is_some_and(|h| {
                h.team_id != installed_team || !h.compatible_with.matches(&metadata.app.version)
            })
        {
            return Err("components_incompatible");
        }
        let recovery = self.recovery_pkg(metadata.edition);
        if prepared.helper_path().is_some() {
            if !recovery.is_file() {
                return Err("wait_update_recovery_pkg");
            }
            SignedPkgBackend::verify_package(&recovery, &installed_team)?;
        }
        let mut pkg = prepared
            .helper_path()
            .map(|p| {
                SignedPkgBackend::new(
                    p.to_owned(),
                    recovery.clone(),
                    installed_team.clone(),
                    self.root.clone(),
                )
            })
            .transpose()?;
        // Authorize before changing the update stage or touching the old healthy App.
        if let Some(pkg) = &mut pkg {
            pkg.authorize()?;
        } else {
            run_administrator(":")?;
        }
        verify_app_version(
            prepared.app_path(),
            &installed_team,
            &metadata.app.bundle_id,
            &metadata.app.version.to_string(),
        )?;
        let now = self.fresh_gate(metadata, uid)?;
        let mut stage = prepared.begin(
            state,
            InstallReadiness {
                clock_trusted: self.clock.trusted(),
                claude_exited: true,
                account_idle: true,
                components_compatible: true,
                now,
            },
        )?;
        let mut journal = UpdateTransaction {
            schema: 1,
            edition: metadata.edition,
            team: installed_team.clone(),
            old_app,
            old_helper: installed.helper_version.clone(),
            app: metadata.version.to_string(),
            helper_changed: pkg.is_some(),
            helper_pending: false,
            app_pending: false,
            committed: false,
            updates: state.clone(),
        };
        self.save(&journal)?;
        let result = (|| {
            loop {
                self.fresh_gate(metadata, uid)?;
                match stage {
                    UStage::InstallingHelper => {
                        journal.helper_pending = true;
                        self.save(&journal)?;
                        let package = pkg.as_mut().ok_or("update_helper_missing")?;
                        package.window = Some(update_window(metadata)?);
                        package.expected_hash = metadata.helper.as_ref().map(|h| h.sha256.clone());
                        package.install_package(
                            prepared.helper_path().ok_or("update_helper_missing")?,
                        )?;
                        let next = if prepared.candidate().install_order()
                            == InstallOrder::HelperThenApp
                        {
                            UStage::InstallingApp
                        } else {
                            UStage::AwaitingHealth
                        };
                        state.advance_install(stage, next)?;
                        stage = next;
                    }
                    UStage::InstallingApp => {
                        journal.app_pending = true;
                        self.save(&journal)?;
                        run_root_operation(
                            &switcher_helper::root_operation::RootOperation::ReplaceApp {
                                source: prepared.app_path().to_owned(),
                                cdhash: prepared.app_cdhash().to_owned(),
                                edition: journal.edition,
                                team: journal.team.clone(),
                                old: journal.old_app.clone(),
                                new: journal.app.clone(),
                                window: update_window(metadata)?,
                            },
                            &journal.team,
                        )?;
                        let next = if prepared.candidate().install_order()
                            == InstallOrder::AppThenHelper
                        {
                            UStage::InstallingHelper
                        } else {
                            UStage::AwaitingHealth
                        };
                        state.advance_install(stage, next)?;
                        stage = next;
                    }
                    UStage::AwaitingHealth => break,
                    _ => return Err("update_install_stage"),
                }
                journal.updates = state.clone();
                self.save(&journal)?;
            }
            let healthy = wait_installed(metadata.edition, uid)?;
            if healthy.helper_version != final_helper.to_string() {
                return Err("updated_helper_version");
            }
            verify_app_version(
                &app,
                &installed_team,
                &metadata.app.bundle_id,
                &metadata.version.to_string(),
            )?;
            if let Some(pkg) = &mut pkg {
                let installer = Installer::new(self.root.clone())?;
                let preview = installer.preview(
                    uid,
                    metadata.edition,
                    Some(metadata.edition),
                    InstallIntent::Install,
                )?;
                pkg.commit_root_record(&preview)?;
            }
            state.advance_install(UStage::AwaitingHealth, UStage::Healthy)?;
            journal.updates = state.clone();
            journal.committed = true;
            self.save(&journal)
        })();
        if let Err(reason) = result {
            state.record_failure(prepared.candidate(), reason, now)?;
            journal.updates = state.clone();
            self.save(&journal)?;
            // The rollback is real, not a state-only success. Failure leaves the journal pending.
            let _ = self.recover(state);
            return Err(reason);
        }
        Ok(())
    }
}
#[cfg(target_os = "macos")]
fn wait_installed(
    edition: Edition,
    uid: u32,
) -> Result<switcher_domain::system::InstallRecord, &'static str> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match validate_installed(edition, uid) {
            Ok(record) => return Ok(record),
            Err(reason) if std::time::Instant::now() >= deadline => return Err(reason),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(250)),
        }
    }
}
#[cfg(target_os = "macos")]
fn installed_app(edition: Edition) -> PathBuf {
    PathBuf::from(match edition {
        Edition::Personal => "/Applications/Claude 切换器 Personal.app",
        Edition::Public => "/Applications/Claude 切换器 Public.app",
    })
}
#[cfg(target_os = "macos")]
fn app_version(path: &Path) -> Result<String, &'static str> {
    let output = std::process::Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(path.join("Contents/Info.plist"))
        .output()
        .map_err(|_| "app_version_read")?;
    if !output.status.success() {
        return Err("app_version_read");
    }
    let version = String::from_utf8(output.stdout)
        .map_err(|_| "app_version_read")?
        .trim()
        .to_owned();
    semver::Version::parse(&version).map_err(|_| "app_version_invalid")?;
    Ok(version)
}
#[cfg(target_os = "macos")]
fn verify_app_version(
    path: &Path,
    team: &str,
    bundle: &str,
    version: &str,
) -> Result<(), &'static str> {
    verify_binary_signature(path, team, bundle)?;
    if app_version(path)? != version {
        return Err("app_version_mismatch");
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn run_administrator(script: &str) -> Result<(), &'static str> {
    let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
    let status = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            &format!("do shell script \"{escaped}\" with administrator privileges"),
        ])
        .status()
        .map_err(|_| "administrator_unavailable")?;
    if status.success() {
        Ok(())
    } else {
        Err("administrator_cancelled_or_failed")
    }
}
#[cfg(target_os = "macos")]
fn shell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
#[cfg(test)]
mod update_transaction_tests {
    use super::*;
    #[test]
    fn region_repair_reuses_only_the_correct_held_lock() {
        let (installer, _) = super::tests::fixture();
        let root = installer.directory();
        let lock = LifecycleLock::try_acquire(root).unwrap();
        assert!(LifecycleLock::try_acquire(root).is_err());
        assert_eq!(repair_with_lock_at(root, &lock, || Ok("sent")), Ok("sent"));
        let other = root.join("other-v2");
        storage::create_private_directory(&other).unwrap();
        let other_lock = LifecycleLock::try_acquire(&other).unwrap();
        assert_eq!(
            repair_with_lock_at(root, &other_lock, || -> Result<(), &'static str> {
                panic!("wrong capability must not send IPC")
            }),
            Err("lifecycle_lock_required")
        );
    }
    fn assert_codesign_inline_argument(generated_shell: bool) {
        use std::os::unix::fs::PermissionsExt;
        let (installer, _) = super::tests::fixture();
        let root = installer.directory();
        let stub = root.join("codesign stub");
        let capture = root.join("codesign-argv");
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\0' \"$@\" > {}\n",
                shell_literal(capture.to_str().unwrap())
            ),
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o700)).unwrap();
        let team = "TESTTEAM01";
        let bundle = "local.claudeswitcher.helper";
        let requirement = format!(
            "=anchor apple generic and certificate leaf[subject.OU] = \"{team}\" and identifier \"{bundle}\""
        );
        let target = root.join("literal '$(touch MUST_NOT_EXIST); helper");
        let read_arguments = || {
            std::fs::read(&capture)
                .unwrap()
                .split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8(s.to_vec()).unwrap())
                .collect::<Vec<_>>()
        };
        let expected_path = if generated_shell {
            let generated = root_operation_script(&target, "{}", team).unwrap();
            let commands: Vec<_> = generated
                .lines()
                .filter(|line| line.starts_with("/usr/bin/codesign "))
                .collect();
            assert_eq!(commands.len(), 1);
            // Execute only this generated line. Never execute the bootstrap/install script.
            let command = commands[0].replacen(
                "/usr/bin/codesign",
                &shell_literal(stub.to_str().unwrap()),
                1,
            );
            assert!(
                std::process::Command::new("/bin/sh")
                    .args(["-c", &command])
                    .env("stage", &target)
                    .status()
                    .unwrap()
                    .success()
            );
            target.join("runner")
        } else {
            verify_binary_signature_with(&target, team, bundle, &stub).unwrap();
            target.clone()
        };
        assert_eq!(
            read_arguments(),
            vec![
                "--verify".to_owned(),
                "--strict".into(),
                "--test-requirement".into(),
                requirement,
                expected_path.to_str().unwrap().into()
            ]
        );
        assert!(!root.join("MUST_NOT_EXIST").exists());
    }
    #[test]
    fn source_local_signature_passes_exact_pin_requirement_as_one_argument() {
        use std::os::unix::fs::PermissionsExt;
        let (installer, _) = super::tests::fixture();
        let root = installer.directory();
        let stub = root.join("codesign-local");
        let capture = root.join("local-argv");
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\0' \"$@\" > {}\n",
                shell_literal(capture.to_str().unwrap())
            ),
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o700)).unwrap();
        let requirement = format!(
            "identifier \"local.claudeswitcher.helper\" and cdhash H\"{}\"",
            "a".repeat(40)
        );
        verify_code_requirement(&root.join("helper"), &requirement, &stub).unwrap();
        let args = std::fs::read(capture).unwrap();
        let args: Vec<_> = args.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
        assert_eq!(args[3], format!("={requirement}").as_bytes());
        assert_eq!(args.len(), 5);
    }
    #[test]
    fn codesign_direct_passes_inline_requirement_as_one_argument() {
        assert_codesign_inline_argument(false);
    }
    #[test]
    fn codesign_generated_shell_passes_inline_requirement_as_one_argument() {
        assert_codesign_inline_argument(true);
    }
    #[test]
    fn shell_sources_remain_literals() {
        let value = "/private/tmp/'$(printf INJECTED); x.app";
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("printf '%s' {}", shell_literal(value))])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, value.as_bytes());
    }
    #[test]
    fn authorization_delay_and_clock_change_reject_expired_installation() {
        let mut clock = crate::time::ClockState::default();
        assert!(validate_update_time(&clock, 100, 200, 199).is_ok());
        assert_eq!(
            validate_update_time(&clock, 100, 200, 200),
            Err("metadata_expired")
        );
        clock.establish_baseline(crate::time::ClockSample {
            utc_secs: 150,
            continuous_ms: 0,
            boot_id: "test".into(),
        });
        assert_eq!(
            validate_update_time(&clock, 100, 200, 150),
            Err("clock_recovery_required")
        );
    }
    #[test]
    fn native_update_blocks_all_account_journals() {
        let (installer, _) = super::tests::fixture();
        let root = installer.directory();
        assert_eq!(ensure_account_idle(root), Ok(()));
        for relative in [
            "account-journal.json",
            "legacy-account-journal.json",
            "migration/in-place-journal.json",
        ] {
            let path = root.join(relative);
            storage::create_private_directory(path.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(root.join("missing"), &path).unwrap();
            assert_eq!(
                ensure_account_idle(root),
                Err("account_recovery_required"),
                "{relative}"
            );
            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            std::fs::remove_file(&path).unwrap();
            storage::atomic_write(&path, b"pending").unwrap();
            assert_eq!(ensure_account_idle(root), Err("account_recovery_required"));
            std::fs::remove_file(path).unwrap();
        }
        let migration = root.join("migration");
        std::fs::remove_dir(&migration).unwrap();
        std::os::unix::fs::symlink(root.join("missing"), &migration).unwrap();
        assert_eq!(ensure_account_idle(root), Err("account_journal"));
        std::fs::remove_file(&migration).unwrap();
        let target = root.join("migration-target");
        storage::create_private_directory(&target).unwrap();
        storage::atomic_write(&target.join("sentinel"), b"unchanged").unwrap();
        std::os::unix::fs::symlink(&target, &migration).unwrap();
        assert_eq!(ensure_account_idle(root), Err("account_journal"));
        assert!(
            std::fs::symlink_metadata(&migration)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read(target.join("sentinel")).unwrap(),
            b"unchanged"
        );
        std::fs::remove_file(&migration).unwrap();
        storage::atomic_write(&migration, b"not a directory").unwrap();
        assert_eq!(ensure_account_idle(root), Err("account_journal"));
        assert_eq!(std::fs::read(&migration).unwrap(), b"not a directory");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_update_rejects_dangling_transaction() {
        let (installer, _) = super::tests::fixture();
        let root = installer.directory();
        let path = root.join("update-install-transaction.json");
        assert_eq!(ensure_update_transaction_idle(&path), Ok(()));
        std::os::unix::fs::symlink(root.join("missing"), &path).unwrap();
        assert_eq!(
            ensure_update_transaction_idle(&path),
            Err("update_transaction_read")
        );
        assert!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        std::fs::remove_file(&path).unwrap();
        let mut journal = UpdateTransaction {
            schema: 1,
            edition: Edition::Public,
            team: "TESTTEAM01".into(),
            old_app: "1.0.0".into(),
            old_helper: "1.0.0".into(),
            app: "1.1.0".into(),
            helper_changed: false,
            helper_pending: false,
            app_pending: true,
            committed: false,
            updates: crate::updates::UpdateState::default(),
        };
        storage::atomic_write(&path, &serde_json::to_vec(&journal).unwrap()).unwrap();
        assert_eq!(
            ensure_update_transaction_idle(&path),
            Err("update_recovery_required")
        );
        journal.committed = true;
        storage::atomic_write(&path, &serde_json::to_vec(&journal).unwrap()).unwrap();
        assert_eq!(ensure_update_transaction_idle(&path), Ok(()));
        let valid_bytes = std::fs::read(&path).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            ensure_update_transaction_idle(&path),
            Err("update_transaction_read")
        );
        assert_eq!(std::fs::read(&path).unwrap(), valid_bytes);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        storage::atomic_write(&path, b"{broken").unwrap();
        assert_eq!(
            ensure_update_transaction_idle(&path),
            Err("update_transaction_read")
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"{broken");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn update_journal_accepts_authenticated_state_larger_than_generic_metadata_limit() {
        let (installer, _) = super::tests::fixture();
        let path = installer
            .directory()
            .join("update-install-transaction.json");
        let mut wire = serde_json::to_value(crate::updates::UpdateState::default()).unwrap();
        wire["candidate"] = serde_json::json!({"wire": vec![0u8; 70_000], "payload_hash":"a".repeat(64), "stage":"installing_app", "install_order":"app_only"});
        let updates = serde_json::from_value(wire).unwrap();
        let j = UpdateTransaction {
            schema: 1,
            edition: Edition::Public,
            team: "TESTTEAM01".into(),
            old_app: "1.0.0".into(),
            old_helper: "1.0.0".into(),
            app: "1.1.0".into(),
            helper_changed: false,
            helper_pending: false,
            app_pending: true,
            committed: false,
            updates,
        };
        let bytes = serde_json::to_vec(&j).unwrap();
        assert!(bytes.len() > 64 * 1024);
        storage::atomic_write(&path, &bytes).unwrap();
        let read = read_update_transaction(&path).unwrap();
        assert!(read.app_pending && !read.committed);
        std::fs::remove_dir_all(installer.directory()).unwrap();
    }
}
