//! Legacy account selection with transparent, recoverable directory transactions.
use crate::accounts::{
    AccountId, AccountStatus, AccountStore, InitializationCopy, ensure_claude_stopped, quit_claude,
};
use crate::storage::{self, LifecycleLock};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use switcher_domain::Edition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyAssessment {
    Unresolved,
    VerifiedPreserved,
    HandlingPlanAvailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    MetadataObserved,
    NoMatchingItems,
    InteractionBlocked,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeychainQuerySummary {
    pub scope_id: String,
    pub os_status: i32,
    pub item_count: usize,
    pub attribute_summary_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeychainMetadataProbe {
    pub schema_version: u32,
    pub status: ProbeStatus,
    pub queries: Vec<KeychainQuerySummary>,
    pub dependency_assessment: DependencyAssessment,
    pub reason: String,
}
impl KeychainMetadataProbe {
    /// Kept for CLI response compatibility. Account switching never queries Keychain.
    pub fn run() -> Self {
        Self {
            schema_version: 1,
            status: ProbeStatus::Unavailable,
            queries: vec![],
            dependency_assessment: DependencyAssessment::VerifiedPreserved,
            reason: "账号切换沿用旧版目录存档，不要求 Keychain 元数据调查".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct MigrationBlocker {
    pub code: String,
    pub message: String,
}
/// UI selects a fixed action; it cannot provide a dependency assessment or a permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationIntent {
    InitializeA,
    AssociateA,
    SelectA,
    SelectB,
    Recover,
}
#[derive(Debug, Clone, Serialize)]
pub struct MigrationReadiness {
    pub ready: bool,
    pub blockers: Vec<MigrationBlocker>,
    pub keychain: KeychainMetadataProbe,
}
#[derive(Debug, Clone)]
pub struct MigrationPreview {
    pub readiness: MigrationReadiness,
    pub accounts: AccountStatus,
}
#[derive(Debug)]
pub enum MigrationError {
    Blocked(MigrationReadiness),
    Invalid(&'static str),
    Operation(&'static str),
}
impl std::fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocked(readiness) => write!(
                formatter,
                "迁移等待：{}",
                readiness
                    .blockers
                    .iter()
                    .map(|item| item.message.as_str())
                    .collect::<Vec<_>>()
                    .join("；")
            ),
            Self::Invalid(message) | Self::Operation(message) => formatter.write_str(message),
        }
    }
}
impl std::error::Error for MigrationError {}

type Result<T> = std::result::Result<T, MigrationError>;
trait Environment: Send + Sync {
    fn quit(&self, owner: u32) -> io::Result<()>;
    fn stopped(&self, owner: u32) -> io::Result<()>;
}
struct NativeEnvironment;
impl Environment for NativeEnvironment {
    fn quit(&self, owner: u32) -> io::Result<()> {
        quit_claude(owner)
    }
    fn stopped(&self, owner: u32) -> io::Result<()> {
        ensure_claude_stopped(owner)
    }
}
pub struct MigrationCoordinator {
    root: PathBuf,
    active: PathBuf,
    owner: u32,
    environment: Box<dyn Environment>,
}
impl MigrationCoordinator {
    pub fn for_current_user(edition: Edition) -> Result<Self> {
        let owner = unsafe { libc::geteuid() };
        let home =
            account_home(owner).map_err(|_| MigrationError::Invalid("无法读取系统用户目录"))?;
        Self::new(
            &home.join(".claude-switcher"),
            &home.join("Library/Application Support/Claude"),
            owner,
            edition,
        )
    }
    pub fn new(root: &Path, active: &Path, owner: u32, _edition: Edition) -> Result<Self> {
        let home =
            account_home(owner).map_err(|_| MigrationError::Invalid("无法读取系统用户目录"))?;
        if owner != unsafe { libc::geteuid() }
            || root != home.join(".claude-switcher")
            || active != home.join("Library/Application Support/Claude")
        {
            return Err(MigrationError::Invalid("账号操作只接受当前用户的固定目录"));
        }
        Ok(Self {
            root: root.into(),
            active: active.into(),
            owner,
            environment: Box::new(NativeEnvironment),
        })
    }
    pub fn readiness(&self) -> Result<MigrationReadiness> {
        Ok(MigrationReadiness {
            ready: true,
            blockers: vec![],
            keychain: KeychainMetadataProbe::run(),
        })
    }
    pub fn readiness_for(&self, _intent: MigrationIntent) -> Result<MigrationReadiness> {
        self.readiness()
    }
    pub fn preview(&self) -> Result<MigrationPreview> {
        let accounts = AccountStore::inspect(&self.root, &self.active, self.owner)
            .map_err(|_| MigrationError::Operation("账号状态无法读取"))?;
        Ok(MigrationPreview {
            readiness: self.readiness()?,
            accounts,
        })
    }
    fn store(&self) -> Result<AccountStore> {
        if !self.root.exists() {
            storage::create_private_directory(&self.root)
                .map_err(|_| MigrationError::Operation("无法创建账号目录"))?;
        }
        if !self.active.exists() {
            storage::create_private_directory(&self.active)
                .map_err(|_| MigrationError::Operation("无法创建 Claude 资料目录"))?;
        }
        AccountStore::new(&self.root, &self.active, self.owner)
            .map_err(|_| MigrationError::Operation("账号目录校验失败"))
    }
    fn select(&self, target: AccountId) -> Result<()> {
        // The legacy command simply opens Claude when this account is already selected.
        if self.root.join("current").exists() {
            let (status, recorded) =
                AccountStore::inspect_with_recorded_account(&self.root, &self.active, self.owner)
                    .map_err(|_| MigrationError::Operation("账号状态无法读取"))?;
            if recorded == Some(target) && !status.recovery_pending {
                return Ok(());
            }
        }
        // A running application gets the same graceful quit request as the original switcher.
        self.environment
            .quit(self.owner)
            .map_err(|_| MigrationError::Operation("Claude 未能退出，请手动退出后重试"))?;
        let store = self.store()?;
        let lock = LifecycleLock::try_acquire(&store.lifecycle_directory())
            .map_err(|_| MigrationError::Operation("其他账号操作正在进行"))?;
        self.recover_in_place(&store, &lock)?;
        store
            .select_legacy_checked(target, &lock, &|| self.environment.stopped(self.owner))
            .map_err(|_| MigrationError::Operation("账号事务未完成；资料已保留，请重试或恢复"))
    }
    pub fn initialize_a(&self) -> Result<()> {
        self.select(AccountId::A)
    }
    pub fn associate_a(&self) -> Result<()> {
        self.select(AccountId::A)
    }
    pub fn select_a(&self) -> Result<()> {
        self.select(AccountId::A)
    }
    pub fn select_b(&self) -> Result<()> {
        self.select(AccountId::B)
    }
    pub fn recover(&self) -> Result<()> {
        self.environment
            .quit(self.owner)
            .map_err(|_| MigrationError::Operation("Claude 未能退出"))?;
        let store = self.store()?;
        let lock = LifecycleLock::try_acquire(&store.lifecycle_directory())
            .map_err(|_| MigrationError::Operation("其他账号操作正在进行"))?;
        self.recover_in_place(&store, &lock)?;
        store
            .recover_legacy_checked(&lock, &|| self.environment.stopped(self.owner))
            .map_err(|_| MigrationError::Operation("事务资料核对失败，已保留全部资料"))
    }
    fn recover_in_place(&self, store: &AccountStore, lock: &LifecycleLock) -> Result<()> {
        let path = self.root.join("v2/migration/in-place-journal.json");
        #[derive(Deserialize)]
        struct PreviousJournal {
            intent: MigrationIntent,
            initialization_copy: Option<InitializationCopy>,
        }
        let journal: PreviousJournal = match storage::read_json(&path) {
            Ok(journal) => journal,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => {
                return Err(MigrationError::Operation(
                    "旧账号事务记录无法读取，已保留资料",
                ));
            }
        };
        if !matches!(
            journal.intent,
            MigrationIntent::InitializeA | MigrationIntent::AssociateA
        ) {
            return Err(MigrationError::Operation("旧账号事务类型不明，已保留资料"));
        }
        if let Some(copy) = journal.initialization_copy {
            store
                .finish_initialization(
                    lock,
                    &copy,
                    &|| self.environment.stopped(self.owner),
                    &mut |_| Ok(()),
                )
                .map_err(|_| MigrationError::Operation("旧账号 A 存档事务恢复失败，已保留资料"))?;
        }
        // In-place journals never exchange the live directory. Retire the old permit after copy completion.
        fs::remove_file(&path).map_err(|_| MigrationError::Operation("旧事务记录无法收尾"))?;
        storage::sync_directory(path.parent().unwrap())
            .map_err(|_| MigrationError::Operation("旧事务记录无法同步"))
    }
    pub fn create_verified_backup(&self) -> Result<()> {
        Err(MigrationError::Invalid(
            "旧版账号切换会自动存档，无需迁移备份或隔离恢复步骤",
        ))
    }
}
pub(crate) fn account_home(owner: u32) -> io::Result<PathBuf> {
    let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 65536];
    let status = unsafe {
        libc::getpwuid_r(
            owner,
            record.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() {
        return Err(io::Error::other("system home lookup failed"));
    }
    let record = unsafe { record.assume_init() };
    if record.pw_dir.is_null() {
        return Err(io::Error::other("system home missing"));
    }
    use std::os::unix::ffi::OsStrExt;
    let home = PathBuf::from(std::ffi::OsStr::from_bytes(
        unsafe { std::ffi::CStr::from_ptr(record.pw_dir) }.to_bytes(),
    ));
    storage::validate_no_symlinks(&home)?;
    Ok(home)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Synthetic {
        quit_calls: Arc<AtomicUsize>,
        running: bool,
    }
    impl Environment for Synthetic {
        fn quit(&self, _: u32) -> io::Result<()> {
            self.quit_calls.fetch_add(1, Ordering::SeqCst);
            if self.running {
                Err(io::Error::other("synthetic quit timeout"))
            } else {
                Ok(())
            }
        }
        fn stopped(&self, _: u32) -> io::Result<()> {
            Ok(())
        }
    }
    struct Fixture {
        base: PathBuf,
        coordinator: MigrationCoordinator,
        calls: Arc<AtomicUsize>,
    }
    impl Fixture {
        fn new(running: bool) -> Self {
            let base = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "legacy-coordinator-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            storage::create_private_directory(&base).unwrap();
            let root = base.join("switcher");
            let active = base.join("Claude");
            storage::create_private_directory(&active).unwrap();
            storage::atomic_write(&active.join("session"), b"A-live").unwrap();
            let calls = Arc::new(AtomicUsize::new(0));
            let coordinator = MigrationCoordinator {
                root,
                active,
                owner: unsafe { libc::geteuid() },
                environment: Box::new(Synthetic {
                    quit_calls: calls.clone(),
                    running,
                }),
            };
            Self {
                base,
                coordinator,
                calls,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    #[test]
    fn first_b_saves_a_then_empty_b_login_and_roundtrip_preserve_latest_data() {
        let fixture = Fixture::new(false);
        let c = &fixture.coordinator;
        assert!(c.readiness().unwrap().ready);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        c.select_b().unwrap();
        assert_eq!(
            fs::read(c.root.join("accounts/A/session")).unwrap(),
            b"A-live"
        );
        assert_eq!(fs::read_dir(&c.active).unwrap().count(), 0);
        assert_eq!(fs::read(c.root.join("current")).unwrap(), b"B\n");
        storage::atomic_write(&c.active.join("session"), b"B-new-login").unwrap();
        c.select_a().unwrap();
        assert_eq!(fs::read(c.active.join("session")).unwrap(), b"A-live");
        c.select_b().unwrap();
        assert_eq!(fs::read(c.active.join("session")).unwrap(), b"B-new-login");
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 3);
        assert!(!c.root.join("v2/migration").exists());
        assert!(c.create_verified_backup().is_err());
    }

    fn assert_unowned_active_preserves_saved_accounts(
        missing_active: bool,
        marker: Option<&[u8]>,
        saved_a: bool,
    ) {
        use std::os::unix::fs::MetadataExt;
        for target in [AccountId::A, AccountId::B] {
            for saved_b in [false, true] {
                if !saved_a && !saved_b {
                    continue;
                }
                let fixture = Fixture::new(false);
                let c = &fixture.coordinator;
                fs::remove_file(c.active.join("session")).unwrap();
                if missing_active {
                    fs::remove_dir(&c.active).unwrap();
                }
                storage::create_private_directory(&c.root).unwrap();
                if let Some(marker) = marker {
                    storage::atomic_write(&c.root.join("current"), marker).unwrap();
                }
                let archives = c.root.join("accounts");
                storage::create_private_directory(&archives).unwrap();
                let mut saved = Vec::new();
                for (name, present, content) in
                    [("A", saved_a, b"A-saved"), ("B", saved_b, b"B-saved")]
                {
                    if present {
                        let archive = archives.join(name);
                        storage::create_private_directory(&archive).unwrap();
                        storage::atomic_write(&archive.join("session"), content).unwrap();
                        let metadata = fs::metadata(&archive).unwrap();
                        saved.push((name, metadata.dev(), metadata.ino(), content));
                    }
                }
                c.select(target).unwrap();
                for (name, device, inode, content) in saved {
                    let archive = archives.join(name);
                    let metadata = fs::metadata(&archive).unwrap();
                    assert_eq!((metadata.dev(), metadata.ino()), (device, inode));
                    assert_eq!(fs::read(archive.join("session")).unwrap(), content);
                }
                assert_eq!(archives.join("A").exists(), saved_a);
                assert_eq!(archives.join("B").exists(), saved_b);
                let expected = match target {
                    AccountId::A if saved_a => Some(b"A-saved".to_vec()),
                    AccountId::B if saved_b => Some(b"B-saved".to_vec()),
                    _ => None,
                };
                assert_eq!(
                    fs::read(c.active.join("session")).ok(),
                    expected,
                    "missing_active={missing_active}, marker={marker:?}, target={target:?}, saved_a={saved_a}, saved_b={saved_b}"
                );
                if expected.is_none() {
                    assert_eq!(fs::read_dir(&c.active).unwrap().count(), 0);
                }
                let marker = match target {
                    AccountId::A => b"A\n",
                    AccountId::B => b"B\n",
                };
                assert_eq!(fs::read(c.root.join("current")).unwrap(), marker);
                assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
                assert!(!c.root.join("v2/legacy-account-journal.json").exists());
            }
        }
    }

    #[test]
    fn f01_invalid_current_marker_does_not_skip_saved_account_loading() {
        for marker in [b"".as_slice(), b"unknown\n".as_slice()] {
            for missing_active in [false, true] {
                assert_unowned_active_preserves_saved_accounts(missing_active, Some(marker), true);
            }
        }
    }

    #[test]
    fn f01_unowned_active_with_only_saved_b_keeps_b_archive() {
        for marker in [None, Some(b"".as_slice()), Some(b"unknown\n".as_slice())] {
            for missing_active in [false, true] {
                assert_unowned_active_preserves_saved_accounts(missing_active, marker, false);
            }
        }
    }

    #[test]
    fn f01_invalid_marker_uses_recorded_v2_for_same_account_shortcut() {
        use std::os::unix::fs::MetadataExt;
        for account in [AccountId::A, AccountId::B] {
            for marker in [b"".as_slice(), b"unknown\n".as_slice()] {
                let fixture = Fixture::new(false);
                let c = &fixture.coordinator;
                c.select(account).unwrap();
                storage::atomic_write(&c.active.join("session"), b"new-live").unwrap();
                storage::atomic_write(&c.root.join("current"), marker).unwrap();
                let before = fs::metadata(&c.active).unwrap();
                c.select(account).unwrap();
                let after = fs::metadata(&c.active).unwrap();
                assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
                assert_eq!(fs::read(c.active.join("session")).unwrap(), b"new-live");
                assert_eq!(fs::read(c.root.join("current")).unwrap(), marker);
                assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
            }
        }
    }

    #[test]
    fn f01_unowned_empty_active_preserves_saved_accounts() {
        assert_unowned_active_preserves_saved_accounts(false, None, true);
    }

    #[test]
    fn f01_unowned_missing_active_preserves_saved_accounts() {
        assert_unowned_active_preserves_saved_accounts(true, None, true);
    }
    #[test]
    fn old_current_wins_and_replaced_live_directory_is_rebound_without_migration() {
        let fixture = Fixture::new(false);
        let c = &fixture.coordinator;
        c.select_a().unwrap();
        fs::rename(&c.active, fixture.base.join("old-live")).unwrap();
        storage::create_private_directory(&c.active).unwrap();
        storage::atomic_write(&c.active.join("session"), b"B-legacy-live").unwrap();
        storage::atomic_write(&c.root.join("current"), b"B\n").unwrap();
        assert_eq!(c.preview().unwrap().accounts.current, Some(AccountId::B));
        c.select_a().unwrap();
        assert_eq!(fs::read(c.active.join("session")).unwrap(), b"A-live");
        assert_eq!(
            fs::read(c.root.join("accounts/B/session")).unwrap(),
            b"B-legacy-live"
        );
    }
    #[test]
    fn previous_in_place_journal_recovers_copy_without_version_keychain_or_backup() {
        let fixture = Fixture::new(false);
        let c = &fixture.coordinator;
        let store = c.store().unwrap();
        let control = c.root.join("v2/migration");
        storage::create_private_directory(&control).unwrap();
        {
            let lock = LifecycleLock::try_acquire(&store.lifecycle_directory()).unwrap();
            store.initialize_with_copy(&lock, &|| Ok(()), &mut |copy| {
                storage::atomic_json(&control.join("in-place-journal.json"), &serde_json::json!({
                    "intent": "InitializeA", "initialization_copy": copy,
                    "installation": "old-unavailable-install", "application": "old-version", "metadata": "old-keychain-hash"
                }))
            }, &mut |phase| {
                if phase == "archive_renamed" { Err(io::Error::other("synthetic crash")) } else { Ok(()) }
            }).unwrap_err();
        }
        c.select_b().unwrap();
        assert!(!control.join("in-place-journal.json").exists());
        assert_eq!(
            fs::read(c.root.join("accounts/A/session")).unwrap(),
            b"A-live"
        );
        assert_eq!(fs::read_dir(&c.active).unwrap().count(), 0);
    }

    #[test]
    fn missing_current_uses_existing_v2_account_selection() {
        let fixture = Fixture::new(false);
        let c = &fixture.coordinator;
        c.select_b().unwrap();
        fs::remove_file(c.root.join("current")).unwrap();
        storage::atomic_write(&c.active.join("session"), b"B-latest").unwrap();
        assert_eq!(c.preview().unwrap().accounts.current, Some(AccountId::B));
        c.select_a().unwrap();
        assert_eq!(
            fs::read(c.root.join("accounts/B/session")).unwrap(),
            b"B-latest"
        );
    }

    #[test]
    fn selecting_current_account_does_not_quit_or_rewrite_live_data() {
        let fixture = Fixture::new(false);
        let c = &fixture.coordinator;
        c.select_a().unwrap();
        storage::atomic_write(&c.active.join("session"), b"A-new-live").unwrap();
        c.select_a().unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fs::read(c.active.join("session")).unwrap(), b"A-new-live");
    }
    #[test]
    fn failed_quit_never_creates_store_or_changes_active_data() {
        let fixture = Fixture::new(true);
        assert!(fixture.coordinator.select_b().is_err());
        assert!(!fixture.coordinator.root.exists());
        assert_eq!(
            fs::read(fixture.coordinator.active.join("session")).unwrap(),
            b"A-live"
        );
    }
    #[test]
    fn first_a_archives_without_requiring_existing_v2_or_backup() {
        let fixture = Fixture::new(false);
        fixture.coordinator.select_a().unwrap();
        assert_eq!(
            fs::read(fixture.coordinator.root.join("accounts/A/session")).unwrap(),
            b"A-live"
        );
        assert_eq!(
            fs::read(fixture.coordinator.active.join("session")).unwrap(),
            b"A-live"
        );
    }
}
