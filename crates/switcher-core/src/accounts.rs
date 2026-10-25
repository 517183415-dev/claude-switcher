//! Legacy account selection and recoverable directory transactions.
mod legacy;
use crate::storage::{self, LifecycleLock};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
type CopyAfterProof = fn(&Path) -> io::Result<()>;
#[cfg(test)]
thread_local! {
    static COPY_AFTER_PROOF: std::cell::Cell<Option<CopyAfterProof>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
type SyntheticProvenance = BTreeMap<(u64, u64), Vec<u8>>;
#[cfg(test)]
thread_local! {
    static COPY_PROVENANCE: std::cell::RefCell<Option<SyntheticProvenance>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(crate) struct CopyProvenanceFixture;
#[cfg(test)]
impl CopyProvenanceFixture {
    pub(crate) fn new() -> Self {
        COPY_PROVENANCE.with(|value| {
            assert!(value.borrow().is_none());
            *value.borrow_mut() = Some(BTreeMap::new());
        });
        Self
    }
    pub(crate) fn set(path: &Path, attribute: Option<&[u8]>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        COPY_PROVENANCE.with(|value| {
            let mut value = value.borrow_mut();
            let map = value.as_mut().unwrap();
            let key = (metadata.dev(), metadata.ino());
            if let Some(attribute) = attribute {
                map.insert(key, attribute.to_vec());
            } else {
                map.remove(&key);
            }
        });
    }
}
#[cfg(test)]
impl Drop for CopyProvenanceFixture {
    fn drop(&mut self) {
        COPY_PROVENANCE.with(|value| *value.borrow_mut() = None);
    }
}
#[cfg(test)]
fn simulate_copy_provenance(from: &Path, to: &Path) -> io::Result<()> {
    fn walk(from: &Path, to: &Path, pairs: &mut Vec<(PathBuf, PathBuf)>) -> io::Result<()> {
        pairs.push((from.to_owned(), to.to_owned()));
        if fs::metadata(from)?.is_dir() {
            let mut entries = fs::read_dir(from)?.collect::<io::Result<Vec<_>>>()?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                walk(&entry.path(), &to.join(entry.file_name()), pairs)?;
            }
        }
        Ok(())
    }
    COPY_PROVENANCE.with(|value| {
        let mut value = value.borrow_mut();
        let Some(map) = value.as_mut() else {
            return Ok(());
        };
        let mut pairs = Vec::new();
        walk(from, to, &mut pairs)?;
        let mut added = false;
        for (source, destination) in pairs {
            if let Some(attribute) = map
                .get(&(
                    fs::symlink_metadata(&source)?.dev(),
                    fs::symlink_metadata(&source)?.ino(),
                ))
                .cloned()
            {
                map.insert(
                    (
                        fs::symlink_metadata(&destination)?.dev(),
                        fs::symlink_metadata(&destination)?.ino(),
                    ),
                    attribute,
                );
            } else if !added {
                map.insert(
                    (
                        fs::symlink_metadata(&destination)?.dev(),
                        fs::symlink_metadata(&destination)?.ino(),
                    ),
                    format!(
                        "synthetic-{}",
                        NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
                    )
                    .into_bytes(),
                );
                added = true;
            }
        }
        Ok(())
    })
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn denied(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountId {
    A,
    B,
}
impl AccountId {
    fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    device: u64,
    inode: u64,
}
fn identity(path: &Path) -> io::Result<Identity> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(denied("expected a real directory"));
    }
    Ok(Identity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
fn optional_identity(path: &Path) -> io::Result<Option<Identity>> {
    match identity(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveState {
    schema: u32,
    account: AccountId,
    generation: u64,
    directory: Identity,
    transaction: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    Prepared,
    SwapIntent,
    Swapped,
    ArchiveIntent,
    Archived,
    StateCommitted,
    Mirrored,
    Completed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: u32,
    transaction: String,
    phase: Phase,
    before: ActiveState,
    target: AccountId,
    original: Identity,
    staged: Identity,
    target_archive: Identity,
    old_archive: Option<Identity>,
    transaction_directory: Identity,
    original_hash: String,
    target_hash: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    staged_hash: Option<String>,
}
pub(crate) fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
pub(crate) fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
impl Journal {
    fn incoming_hash(&self) -> &str {
        self.staged_hash.as_deref().unwrap_or(&self.target_hash)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InitializationCopy {
    transaction: String,
    directory: Identity,
    hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStatus {
    pub current: Option<AccountId>,
    pub a_saved: bool,
    pub b_saved: bool,
    pub recovery_pending: bool,
}
/// Pending user/system repairs outlive an OS lock after a crash. Account data must wait.
pub(crate) fn ensure_no_pending_repairs(root: &Path) -> io::Result<()> {
    for relative in ["v2/region-repair.json", "v2/user-repairs/journal.json"] {
        match fs::symlink_metadata(root.join(relative)) {
            Ok(_) => {
                return Err(denied(
                    "地区或用户配置修复尚未完成；先恢复修复事务，再操作账号",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(denied("无法安全确认地区或用户配置修复状态")),
        }
    }
    Ok(())
}
/// Preview is opaque. Association rechecks both identity and content before committing.
#[derive(Debug, Clone)]
pub struct AssociationPreview {
    active: Identity,
    archived: Identity,
    fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct AccountStore {
    root: PathBuf,
    active: PathBuf,
    owner: u32,
}
impl AccountStore {
    /// Reads account metadata without creating directories or trusting the legacy current mirror.
    pub fn inspect(root: &Path, active: &Path, owner: u32) -> io::Result<AccountStatus> {
        Self::inspect_with_recorded_account(root, active, owner).map(|(status, _)| status)
    }
    /// Inspects metadata without creating account markers, directories, or recovery state.
    /// `AccountStatus.current` may be a display default; the second value is recorded ownership.
    /// A missing recorded owner must not be inferred from the display default.
    pub fn inspect_with_recorded_account(
        root: &Path,
        active: &Path,
        owner: u32,
    ) -> io::Result<(AccountStatus, Option<AccountId>)> {
        if owner != unsafe { libc::geteuid() } {
            return Err(denied("account owner is not the current user"));
        }
        if !root.is_absolute()
            || !active.is_absolute()
            || root.starts_with(active)
            || active.starts_with(root)
        {
            return Err(denied("invalid or overlapping account roots"));
        }
        let store = Self {
            root: root.to_owned(),
            active: active.to_owned(),
            owner,
        };
        if optional_identity(root)?.is_none() {
            storage::validate_no_symlinks(
                root.parent()
                    .ok_or_else(|| invalid("missing root parent"))?,
            )?;
            return Ok((
                AccountStatus {
                    current: Some(AccountId::A),
                    a_saved: false,
                    b_saved: false,
                    recovery_pending: false,
                },
                None,
            ));
        }
        store.check_directory(root)?;
        for directory in [store.v2(), store.transactions()] {
            if optional_identity(&directory)?.is_some() {
                store.check_directory(&directory)?;
            }
        }
        for account in [AccountId::A, AccountId::B] {
            if optional_identity(&store.archive(account))?.is_some() {
                storage::validate_no_symlinks(&store.archive(account))?;
            }
        }
        Ok((store.status()?, store.recorded_account()?))
    }
    /// Root and active directory must be explicit, disjoint, real directories.
    pub fn new(root: &Path, active: &Path, owner: u32) -> io::Result<Self> {
        if owner != unsafe { libc::geteuid() } {
            return Err(denied("account owner is not the current user"));
        }
        storage::validate_no_symlinks(root)?;
        storage::validate_no_symlinks(active)?;
        if root.starts_with(active) || active.starts_with(root) {
            return Err(denied("overlapping account roots"));
        }
        let store = Self {
            root: root.to_owned(),
            active: active.to_owned(),
            owner,
        };
        store.check_directory(root)?;
        identity(active)?;
        ensure_no_pending_repairs(root)?;
        let archives = store.archives();
        if optional_identity(&archives)?.is_none() {
            storage::create_private_directory(&archives)?;
        }
        // Legacy data permissions and ACLs remain unchanged; control directories stay private.
        storage::validate_no_symlinks(&archives)?;
        for path in [store.v2(), store.transactions()] {
            storage::create_private_directory(&path)?;
        }
        Ok(store)
    }
    fn archives(&self) -> PathBuf {
        self.root.join("accounts")
    }
    fn archive(&self, account: AccountId) -> PathBuf {
        self.archives().join(account.name())
    }
    pub fn lifecycle_directory(&self) -> PathBuf {
        self.v2()
    }
    fn v2(&self) -> PathBuf {
        self.root.join("v2")
    }
    fn transactions(&self) -> PathBuf {
        self.v2().join("transactions")
    }
    fn state_path(&self) -> PathBuf {
        self.v2().join("active-state.json")
    }
    fn journal_path(&self) -> PathBuf {
        self.v2().join("account-journal.json")
    }
    fn check_directory(&self, path: &Path) -> io::Result<()> {
        storage::validate_no_symlinks(path)?;
        let metadata = fs::metadata(path)?;
        if metadata.uid() != self.owner || metadata.mode() & 0o022 != 0 {
            return Err(denied("unsafe account directory ownership or mode"));
        }
        if metadata.dev() != identity(&self.root)?.device {
            return Err(denied("account directory crosses volumes"));
        }
        access_control(path)?;
        Ok(())
    }
    fn state(&self) -> io::Result<Option<ActiveState>> {
        let state: ActiveState = match storage::read_json(&self.state_path()) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if state.schema != 1 || state.generation == 0 {
            return Err(invalid("unsupported active state"));
        }
        Ok(Some(state))
    }
    /// Legacy `current` remains the user-visible account selection. Old v2 state is a fallback.
    fn selected_account(&self) -> io::Result<AccountId> {
        Ok(self.recorded_account()?.unwrap_or(AccountId::A))
    }
    /// A display default is not proof that the live directory belongs to an account.
    fn recorded_account(&self) -> io::Result<Option<AccountId>> {
        let current = self.root.join("current");
        if current.try_exists()? {
            self.check_directory(&self.root)?;
            let metadata = fs::symlink_metadata(&current)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.uid() != self.owner
                || metadata.mode() & 0o022 != 0
            {
                return Err(denied("unsafe current account marker"));
            }
            match fs::read_to_string(current)?.trim() {
                "A" => return Ok(Some(AccountId::A)),
                "B" => return Ok(Some(AccountId::B)),
                _ => {}
            }
        }
        Ok(self.state()?.map(|state| state.account))
    }
    pub fn status(&self) -> io::Result<AccountStatus> {
        Ok(AccountStatus {
            current: Some(self.selected_account()?),
            a_saved: optional_identity(&self.archive(AccountId::A))?.is_some(),
            b_saved: optional_identity(&self.archive(AccountId::B))?.is_some(),
            recovery_pending: self.v2().join("legacy-account-journal.json").try_exists()?
                || self.journal_path().try_exists()?
                || self
                    .root
                    .join("v2/migration/in-place-journal.json")
                    .try_exists()?,
        })
    }
    fn no_journal(&self) -> io::Result<()> {
        if self.journal_path().try_exists()? {
            return Err(invalid(
                "account recovery required before mutation or launch",
            ));
        }
        Ok(())
    }
    fn mirror(&self, account: AccountId) -> io::Result<()> {
        storage::atomic_write(
            &self.root.join("current"),
            format!("{}\n", account.name()).as_bytes(),
        )
    }
    fn check_lock(&self, lock: &LifecycleLock) -> io::Result<()> {
        ensure_no_pending_repairs(&self.root)?;
        self.check_directory(&self.root)?;
        self.check_directory(&self.archives())?;
        self.check_directory(&self.active)?;
        self.check_directory(&self.transactions())?;
        if !lock.protects(&self.v2()) {
            return Err(denied("lifecycle lock belongs to another store"));
        }
        Ok(())
    }
    /// Creates the first A archive without replacing an existing archive.
    pub fn initialize_a(&self) -> io::Result<()> {
        let lock = LifecycleLock::try_acquire(&self.v2())?;
        self.initialize_checked(&lock, &|| ensure_claude_stopped(self.owner))
    }
    fn initialize_checked(
        &self,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        self.initialize_with_copy(lock, stopped, &mut |_| Ok(()), &mut |_| Ok(()))
    }
    pub(crate) fn initialize_with_copy(
        &self,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
        bind: &mut dyn FnMut(&InitializationCopy) -> io::Result<()>,
        hook: &mut dyn FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        stopped()?;
        self.no_journal()?;
        if self.state()?.is_some() {
            return Err(invalid("account state already initialized"));
        }
        if optional_identity(&self.archive(AccountId::A))?.is_some()
            || optional_identity(&self.archive(AccountId::B))?.is_some()
        {
            return Err(invalid(
                "existing archives require explicit association or manual recovery",
            ));
        }
        let tx = self.new_transaction()?;
        let staging = tx.join("incoming");
        let hash = self.copy_verified(&self.active, &staging)?;
        let copy = InitializationCopy {
            transaction: tx.file_name().unwrap().to_str().unwrap().to_owned(),
            directory: identity(&staging)?,
            hash,
        };
        hook("copy_completed")?;
        // The coordinator must persist the full copy binding before any rename.
        bind(&copy)?;
        hook("copy_bound")?;
        self.finish_initialization(lock, &copy, stopped, hook)
    }
    pub(crate) fn validate_initialization_copy(&self, copy: &InitializationCopy) -> io::Result<()> {
        if !valid_hash(&copy.hash)
            || copy.transaction.len() > 80
            || !copy.transaction.starts_with("tx-")
            || !copy
                .transaction
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(invalid("invalid initialization copy binding"));
        }
        let staging = self.transactions().join(&copy.transaction).join("incoming");
        let archive = self.archive(AccountId::A);
        let path = match (optional_identity(&staging)?, optional_identity(&archive)?) {
            (Some(staged), None) if staged == copy.directory => staging,
            (None, Some(archived)) if archived == copy.directory => archive,
            _ => return Err(invalid("initialization copy identity conflict")),
        };
        if self.fingerprint(&path)?.0 != copy.hash {
            return Err(invalid("initialization copy changed"));
        }
        Ok(())
    }
    pub(crate) fn finish_initialization(
        &self,
        lock: &LifecycleLock,
        copy: &InitializationCopy,
        stopped: &dyn Fn() -> io::Result<()>,
        hook: &mut dyn FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        stopped()?;
        self.no_journal()?;
        self.validate_initialization_copy(copy)?;
        let tx = self.transactions().join(&copy.transaction);
        let staging = tx.join("incoming");
        if optional_identity(&staging)?.is_some() {
            if self.state()?.is_some() {
                return Err(invalid("state precedes initialization archive"));
            }
            fs::rename(&staging, self.archive(AccountId::A))?;
            storage::sync_directory(&self.archives())?;
            storage::sync_directory(&tx)?;
        }
        hook("archive_renamed")?;
        let state = ActiveState {
            schema: 1,
            account: AccountId::A,
            generation: 1,
            directory: identity(&self.active)?,
            transaction: None,
        };
        match self.state()? {
            Some(existing) if existing != state => {
                return Err(invalid("initialization state conflicts"));
            }
            None => storage::atomic_json(&self.state_path(), &state)?,
            _ => {}
        }
        hook("state_written")?;
        self.mirror(AccountId::A)?;
        if optional_identity(&tx)?.is_some() {
            fs::remove_dir(&tx)?;
        }
        storage::sync_directory(&self.transactions())
    }
    /// For an existing A archive with missing current/state. No association is inferred.
    pub fn preview_association_a(&self) -> io::Result<AssociationPreview> {
        self.no_journal()?;
        if self.state()?.is_some() {
            return Err(invalid("association requires missing active state"));
        }
        let active_hash = self.fingerprint(&self.active)?.0;
        if active_hash != self.fingerprint(&self.archive(AccountId::A))?.0 {
            return Err(invalid(
                "active data differs from A archive; manual recovery required",
            ));
        }
        Ok(AssociationPreview {
            active: identity(&self.active)?,
            archived: identity(&self.archive(AccountId::A))?,
            fingerprint: active_hash,
        })
    }
    pub fn associate_a(&self, preview: &AssociationPreview) -> io::Result<()> {
        let lock = LifecycleLock::try_acquire(&self.v2())?;
        self.associate_checked(&lock, preview, &|| ensure_claude_stopped(self.owner))
    }
    fn associate_checked(
        &self,
        lock: &LifecycleLock,
        preview: &AssociationPreview,
        stopped: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        stopped()?;
        let fresh = self.preview_association_a()?;
        if fresh.active != preview.active
            || fresh.archived != preview.archived
            || fresh.fingerprint != preview.fingerprint
        {
            return Err(invalid("association preview expired"));
        }
        storage::atomic_json(
            &self.state_path(),
            &ActiveState {
                schema: 1,
                account: AccountId::A,
                generation: 1,
                directory: fresh.active,
                transaction: None,
            },
        )?;
        self.mirror(AccountId::A)
    }
    pub fn switch_to(&self, target: AccountId) -> io::Result<()> {
        let lock = LifecycleLock::try_acquire(&self.v2())?;
        self.switch_to_with_lock(target, &lock)
    }
    pub fn switch_to_with_lock(&self, target: AccountId, lock: &LifecycleLock) -> io::Result<()> {
        quit_claude(self.owner)?;
        self.select_legacy_checked(target, lock, &|| ensure_claude_stopped(self.owner))
    }
    /// Retains the transaction journal, but requires no migration or version-specific permit.
    pub(crate) fn select_legacy_checked(
        &self,
        target: AccountId,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        legacy::select(self, target, lock, stopped)
    }
    pub(crate) fn recover_legacy_checked(
        &self,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        legacy::recover(self, lock, stopped)?;
        if self.journal_path().try_exists()? {
            self.recover_checked(lock, stopped)?;
        }
        Ok(())
    }
    #[cfg(test)]
    fn switch_checked(
        &self,
        target: AccountId,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
        hook: &mut dyn FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.switch_guarded(target, lock, stopped, &|| Ok(()), hook)
    }
    /// The coordinator revalidates its private permit after copying and immediately before exchange.
    #[cfg(test)]
    pub(crate) fn switch_guarded(
        &self,
        target: AccountId,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
        before_exchange: &dyn Fn() -> io::Result<()>,
        hook: &mut dyn FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        stopped()?;
        self.no_journal()?;
        let before = self
            .state()?
            .ok_or_else(|| invalid("initialize or associate account A first"))?;
        if before.directory != identity(&self.active)? {
            return Err(invalid("active identity differs from committed state"));
        }
        if before.account == target {
            return self.mirror(target);
        }
        let (original_hash, _) = self.fingerprint(&self.active)?;
        let (target_hash, _) = self.fingerprint(&self.archive(target))?;
        let target_archive = identity(&self.archive(target))?;
        let old_archive = optional_identity(&self.archive(before.account))?;
        if old_archive.is_some() {
            self.fingerprint(&self.archive(before.account))?;
        }
        let tx = self.new_transaction()?;
        let incoming = tx.join("incoming");
        let staged_hash = self.copy_verified(&self.archive(target), &incoming)?;
        let mut journal = Journal {
            schema: 1,
            transaction: tx.file_name().unwrap().to_str().unwrap().to_owned(),
            phase: Phase::Prepared,
            original: before.directory,
            staged: identity(&incoming)?,
            target_archive,
            old_archive,
            transaction_directory: identity(&tx)?,
            before,
            target,
            original_hash,
            target_hash,
            staged_hash: Some(staged_hash),
        };
        self.write_journal(&journal)?;
        hook("prepared")?;
        journal.phase = Phase::SwapIntent;
        self.write_journal(&journal)?;
        hook("swap_intent")?;
        stopped()?;
        self.validate_pre_swap(&journal, &incoming)?;
        before_exchange()?;
        exchange(&self.active, &incoming)?;
        self.sync_exchange(&incoming)?;
        hook("after_swap")?;
        journal.phase = Phase::Swapped;
        self.write_journal(&journal)?;
        hook("swapped")?;
        self.finish(&mut journal, stopped, hook)
    }
    pub fn recover(&self) -> io::Result<()> {
        let lock = LifecycleLock::try_acquire(&self.v2())?;
        self.recover_with_lock(&lock)
    }
    pub fn recover_with_lock(&self, lock: &LifecycleLock) -> io::Result<()> {
        self.recover_legacy_checked(lock, &|| ensure_claude_stopped(self.owner))
    }
    pub(crate) fn recover_checked(
        &self,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        stopped()?;
        let mut journal: Journal = match storage::read_json(&self.journal_path()) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if let Some(state) = self.state()? {
                    if identity(&self.active)? != state.directory {
                        return Err(invalid("active state identity conflict"));
                    }
                    self.mirror(state.account)?;
                }
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        self.validate_journal(&journal)?;
        let tx = self.transactions().join(&journal.transaction);
        let incoming = tx.join("incoming");
        let current = identity(&self.active)?;
        if current == journal.original {
            if self.state()?.as_ref() != Some(&journal.before)
                || identity(&tx)? != journal.transaction_directory
                || optional_identity(&incoming)? != Some(journal.staged)
            {
                return Err(invalid("pre-swap recovery identity conflict"));
            }
            self.validate_pre_swap(&journal, &incoming)?;
            self.remove_staging(&journal, Some(journal.staged))?;
            self.mirror(journal.before.account)?;
            return self.clear_journal();
        }
        if current != journal.staged {
            return Err(invalid(
                "recovery requires manual directory identity inspection",
            ));
        }
        self.finish(&mut journal, stopped, &mut |_| Ok(()))
    }
    fn validate_journal(&self, journal: &Journal) -> io::Result<()> {
        if journal.schema != 1
            || journal.before.schema != 1
            || journal.before.generation == 0
            || journal.before.account == journal.target
            || journal.before.directory != journal.original
            || journal.original == journal.staged
            || journal.transaction.len() > 80
            || !journal.transaction.starts_with("tx-")
            || !journal
                .transaction
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || !valid_hash(&journal.original_hash)
            || !valid_hash(&journal.target_hash)
            || journal
                .staged_hash
                .as_ref()
                .is_some_and(|hash| !valid_hash(hash))
        {
            return Err(invalid(
                "invalid account journal; preserve directories for manual recovery",
            ));
        }
        Ok(())
    }
    fn validate_pre_swap(&self, journal: &Journal, incoming: &Path) -> io::Result<()> {
        if identity(&self.active)? != journal.original
            || identity(incoming)? != journal.staged
            || identity(&self.archive(journal.target))? != journal.target_archive
            || optional_identity(&self.archive(journal.before.account))? != journal.old_archive
            || self.fingerprint(&self.active)?.0 != journal.original_hash
            || self.fingerprint(incoming)?.0 != journal.incoming_hash()
            || self.fingerprint(&self.archive(journal.target))?.0 != journal.target_hash
        {
            return Err(invalid("prepared account data changed"));
        }
        Ok(())
    }
    fn expected_state(&self, journal: &Journal) -> io::Result<ActiveState> {
        Ok(ActiveState {
            schema: 1,
            account: journal.target,
            generation: journal
                .before
                .generation
                .checked_add(1)
                .ok_or_else(|| invalid("generation overflow"))?,
            directory: journal.staged,
            transaction: Some(journal.transaction.clone()),
        })
    }
    fn finish(
        &self,
        journal: &mut Journal,
        stopped: &dyn Fn() -> io::Result<()>,
        hook: &mut dyn FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.validate_journal(journal)?;
        let tx = self.transactions().join(&journal.transaction);
        let incoming = tx.join("incoming");
        let archive = self.archive(journal.before.account);
        let expected = self.expected_state(journal)?;
        let state = self
            .state()?
            .ok_or_else(|| invalid("missing committed pre-switch state"))?;
        if state != journal.before && state != expected {
            return Err(invalid("journal conflicts with committed state"));
        }
        if identity(&self.active)? != journal.staged
            || self.fingerprint(&self.active)?.0 != journal.incoming_hash()
            || identity(&self.archive(journal.target))? != journal.target_archive
            || self.fingerprint(&self.archive(journal.target))?.0 != journal.target_hash
        {
            return Err(invalid("target directory changed during recovery"));
        }
        let archive_identity = optional_identity(&archive)?;
        if archive_identity != Some(journal.original) {
            if state != journal.before
                || identity(&tx)? != journal.transaction_directory
                || optional_identity(&incoming)? != Some(journal.original)
                || archive_identity != journal.old_archive
                || self.fingerprint(&incoming)?.0 != journal.original_hash
            {
                return Err(invalid("outgoing archive identity conflict"));
            }
            journal.phase = Phase::ArchiveIntent;
            self.write_journal(journal)?;
            hook("archive_intent")?;
            stopped()?;
            if journal.old_archive.is_some() {
                exchange(&incoming, &archive)?;
            } else {
                fs::rename(&incoming, &archive)?;
            }
            storage::sync_directory(&tx)?;
            storage::sync_directory(&self.archives())?;
            hook("after_archive")?;
        }
        if self.fingerprint(&archive)?.0 != journal.original_hash {
            return Err(invalid("outgoing archive content conflict"));
        }
        journal.phase = Phase::Archived;
        self.write_journal(journal)?;
        hook("archived")?;
        stopped()?;
        storage::atomic_json(&self.state_path(), &expected)?;
        journal.phase = Phase::StateCommitted;
        self.write_journal(journal)?;
        hook("state_committed")?;
        self.mirror(journal.target)?;
        journal.phase = Phase::Mirrored;
        self.write_journal(journal)?;
        hook("mirrored")?;
        journal.phase = Phase::Completed;
        self.write_journal(journal)?;
        hook("completed")?;
        if tx.try_exists()? {
            self.remove_staging(journal, journal.old_archive)?;
        }
        hook("cleaned")?;
        self.clear_journal()
    }
    fn write_journal(&self, journal: &Journal) -> io::Result<()> {
        storage::atomic_json(&self.journal_path(), journal)
    }
    fn clear_journal(&self) -> io::Result<()> {
        fs::remove_file(self.journal_path())?;
        storage::sync_directory(&self.v2())
    }
    fn remove_staging(&self, journal: &Journal, expected: Option<Identity>) -> io::Result<()> {
        let tx = self.transactions().join(&journal.transaction);
        let incoming = tx.join("incoming");
        if identity(&tx)? != journal.transaction_directory
            || optional_identity(&incoming)? != expected
        {
            return Err(invalid("staging cleanup identity conflict"));
        }
        // A complete scan rejects links and foreign-owned data before recursive removal.
        self.fingerprint(&tx)?;
        let entries = fs::read_dir(&tx)?.collect::<Result<Vec<_>, _>>()?;
        if entries.iter().any(|entry| entry.file_name() != "incoming") {
            return Err(invalid("unexpected staging entry"));
        }
        if expected.is_some() {
            fs::remove_dir_all(&incoming)?;
        }
        fs::remove_dir(&tx)?;
        storage::sync_directory(&self.transactions())
    }
    fn new_transaction(&self) -> io::Result<PathBuf> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let path = self.transactions().join(format!(
            "tx-{nanos}-{}-{}",
            std::process::id(),
            NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
        ));
        if path.try_exists()? {
            return Err(invalid("transaction name collision"));
        }
        storage::create_private_directory(&path)?;
        Ok(path)
    }
    #[cfg(test)]
    fn sync_exchange(&self, incoming: &Path) -> io::Result<()> {
        storage::sync_directory(self.active.parent().unwrap())?;
        storage::sync_directory(incoming.parent().unwrap())
    }
    pub(crate) fn copy_verified(&self, from: &Path, to: &Path) -> io::Result<String> {
        let source_identity = identity(from)?;
        let source_metadata = copy_metadata(from)?;
        let (hash, bytes) = self.fingerprint(from)?;
        ensure_space(
            to.parent().unwrap(),
            bytes
                .checked_add(1024 * 1024)
                .ok_or_else(|| invalid("account size overflow"))?,
        )?;
        if to.try_exists()? {
            return Err(invalid("copy destination already exists"));
        }
        let output = Command::new("/usr/bin/ditto")
            .args(["--rsrc", "--extattr", "--acl"])
            .arg(from)
            .arg(to)
            .env_clear()
            .env("LC_ALL", "C")
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                "account copy failed; staged data retained",
            ));
        }
        #[cfg(test)]
        simulate_copy_provenance(from, to)?;
        let target_identity = identity(to)?;
        if source_identity == target_identity {
            return Err(invalid("copy destination reuses the source identity"));
        }
        // Bind the actual destination before proving its relation to the source.
        let target_fingerprint = self.fingerprint(to)?;
        let target_metadata = copy_metadata(to)?;
        verify_copy_tree(from, to)?;
        #[cfg(test)]
        COPY_AFTER_PROOF.with(|hook| hook.take().map_or(Ok(()), |hook| hook(to)))?;
        if self.fingerprint(to)? != target_fingerprint
            || copy_metadata(to)? != target_metadata
            || self.fingerprint(from)?.0 != hash
            || copy_metadata(from)? != source_metadata
            || identity(from)? != source_identity
            || identity(to)? != target_identity
        {
            return Err(invalid("copy source or destination changed during proof"));
        }
        sync_tree(to)?;
        storage::sync_directory(to.parent().unwrap())?;
        Ok(target_fingerprint.0)
    }
    pub(crate) fn fingerprint(&self, path: &Path) -> io::Result<(String, u64)> {
        self.check_directory(path)?;
        let mut hasher = Sha256::new();
        let mut bytes = 0u64;
        self.scan(path, path, &mut hasher, &mut bytes, 0)?;
        Ok((
            hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            bytes,
        ))
    }
    fn scan(
        &self,
        root: &Path,
        path: &Path,
        hasher: &mut Sha256,
        bytes: &mut u64,
        migration_source: u32,
    ) -> io::Result<()> {
        let relative_path = path.strip_prefix(root).map_err(io::Error::other)?;
        if migration_source != 0
            && (relative_path == Path::new("v2/migration")
                || relative_path == Path::new("v2/lifecycle.lock"))
        {
            return Ok(());
        }
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || (!metadata.is_dir() && !metadata.is_file())
            || metadata.uid() != self.owner
            || metadata.mode() & 0o022 != 0
            || metadata.dev() != identity(&self.root)?.device
            || (metadata.is_file() && metadata.nlink() != 1)
        {
            return Err(denied("unsafe account tree entry"));
        }
        if migration_source != 0 && relative_path == Path::new("v2/guardian-snapshot.lock") {
            access_control(path)?;
            if !metadata.is_file() || metadata.mode() & 0o077 != 0 || metadata.len() != 0 {
                return Err(denied("unsafe Guardian snapshot lock"));
            }
            return Ok(());
        }
        let relative = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .as_os_str()
            .as_bytes();
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative);
        hasher.update([u8::from(metadata.is_dir())]);
        hasher.update((metadata.mode() & 0o7777).to_le_bytes());
        hasher.update(metadata.uid().to_le_bytes());
        let acl = access_control(path)?;
        hasher.update((acl.len() as u64).to_le_bytes());
        *bytes = bytes
            .checked_add(acl.len() as u64)
            .and_then(|value| value.checked_add(4096))
            .ok_or_else(|| invalid("account metadata size overflow"))?;
        hasher.update(acl);
        let attrs = extended_attributes(path)?;
        let encoded =
            serde_json::to_vec(&attrs.into_iter().collect::<Vec<_>>()).map_err(io::Error::other)?;
        hasher.update((encoded.len() as u64).to_le_bytes());
        *bytes = bytes
            .checked_add(encoded.len() as u64)
            .ok_or_else(|| invalid("account attribute size overflow"))?;
        hasher.update(encoded);
        // Only the exact live-root health payload is volatile. Metadata, ACL and xattrs bind.
        if migration_source == 2 && relative_path == Path::new("v2/guardian-health.json") {
            if !metadata.is_file() || metadata.mode() & 0o077 != 0 {
                return Err(denied("unsafe Guardian health"));
            }
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(path)?;
            let opened = file.metadata()?;
            if opened.ino() != metadata.ino()
                || opened.dev() != metadata.dev()
                || opened.mode() != metadata.mode()
                || opened.uid() != metadata.uid()
                || opened.nlink() != 1
            {
                return Err(invalid("Guardian health changed during scan"));
            }
            return Ok(());
        }
        if metadata.is_dir() {
            let mut children = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
            children.sort_by_key(|entry| entry.file_name());
            for entry in children {
                self.scan(root, &entry.path(), hasher, bytes, migration_source)?;
            }
        } else {
            *bytes = bytes
                .checked_add(metadata.len())
                .ok_or_else(|| invalid("account size overflow"))?;
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?;
            let opened = file.metadata()?;
            if opened.ino() != metadata.ino() || opened.dev() != metadata.dev() {
                return Err(invalid("account file changed during scan"));
            }
            hasher.update(metadata.len().to_le_bytes());
            let mut buffer = [0u8; 65536];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
        }
        Ok(())
    }
}
use std::os::unix::fs::OpenOptionsExt;

/// Ask Claude to quit and wait up to 15 seconds before any account directory write.
pub fn quit_claude(owner: u32) -> io::Result<()> {
    quit_claude_with(
        &|| ensure_claude_stopped(owner),
        &|| {
            let mut child = Command::new("/usr/bin/osascript")
                .args(["-e", "tell application \"Claude\" to quit"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
            // AppleScript may wait for an unresponsive application. Bound that request too.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            while child.try_wait()?.is_none() {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(())
        },
        &|| std::thread::sleep(std::time::Duration::from_secs(1)),
    )
}
fn quit_claude_with(
    stopped: &dyn Fn() -> io::Result<()>,
    request: &dyn Fn() -> io::Result<()>,
    wait: &dyn Fn(),
) -> io::Result<()> {
    match stopped() {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
        Err(error) => return Err(error),
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    request()?;
    for _ in 0..15 {
        match stopped() {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        wait();
    }
    stopped()
}

/// Fail closed on process-enumeration failure. No credentials or process arguments are read.
pub fn ensure_claude_stopped(owner: u32) -> io::Result<()> {
    let output = Command::new("/bin/ps")
        .args(["-ww", "-axo", "uid=,comm="])
        .env_clear()
        .env("LC_ALL", "C")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("cannot inspect Claude processes"));
    }
    ensure_claude_stopped_listing(owner, &output.stdout)
}

// Share the production parser with isolated tests; tests never create or run an app bundle.
fn ensure_claude_stopped_listing(owner: u32, listing: &[u8]) -> io::Result<()> {
    let listing = std::str::from_utf8(listing).map_err(|_| invalid("invalid process metadata"))?;
    for line in listing.lines() {
        let line = line.trim_start();
        let split = line
            .find(char::is_whitespace)
            .ok_or_else(|| invalid("invalid process record"))?;
        let uid: u32 = line[..split]
            .parse()
            .map_err(|_| invalid("invalid process owner"))?;
        let command = line[split..].trim_start();
        if uid == owner && (command.contains("/Claude.app/Contents/") || command == "Claude") {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Claude must be completely stopped before account changes",
            ));
        }
    }
    Ok(())
}
fn path_cstring(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| invalid("NUL in path"))
}
#[cfg(target_os = "macos")]
fn exchange(left: &Path, right: &Path) -> io::Result<()> {
    let left = path_cstring(left)?;
    let right = path_cstring(right)?;
    // Both paths were checked for ownership, symlinks and a shared device. The kernel exchanges entries atomically.
    let result = unsafe { libc::renamex_np(left.as_ptr(), right.as_ptr(), libc::RENAME_SWAP) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(not(target_os = "macos"))]
fn exchange(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic account exchange requires macOS",
    ))
}
fn ensure_space(directory: &Path, required: u64) -> io::Result<()> {
    let path = path_cstring(directory)?;
    let mut statistics = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), statistics.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let statistics = unsafe { statistics.assume_init() };
    let available = (statistics.f_bavail as u64)
        .checked_mul(statistics.f_frsize)
        .ok_or_else(|| invalid("free space overflow"))?;
    if available < required {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "insufficient space for staged account copy",
        ));
    }
    Ok(())
}
// The ordinary account digest is unchanged; copy proof also binds metadata and identities.
#[derive(Debug, PartialEq, Eq)]
struct CopyMetadata {
    relative: PathBuf,
    device: u64,
    inode: u64,
    gid: u32,
    links: u64,
    mtime: i64,
    mtime_nsec: i64,
}
fn copy_metadata(path: &Path) -> io::Result<Vec<CopyMetadata>> {
    fn walk(root: &Path, path: &Path, output: &mut Vec<CopyMetadata>) -> io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("copy source symlink"));
        }
        output.push(CopyMetadata {
            relative: path.strip_prefix(root).unwrap().to_owned(),
            device: metadata.dev(),
            inode: metadata.ino(),
            gid: metadata.gid(),
            links: metadata.nlink(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
        });
        if metadata.is_dir() {
            let mut children = fs::read_dir(path)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<io::Result<Vec<_>>>()?;
            children.sort();
            for child in children {
                walk(root, &child, output)?;
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    walk(path, path, &mut output)?;
    Ok(output)
}

/// A one-time proof for a freshly created, distinct-inode copy. Never use as a durable binding.
fn verify_copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    let left = fs::symlink_metadata(from)?;
    let right = fs::symlink_metadata(to)?;
    if left.file_type().is_symlink()
        || right.file_type().is_symlink()
        || (!left.is_dir() && !left.is_file())
        || left.is_dir() != right.is_dir()
        || left.dev() != right.dev()
        || left.ino() == right.ino()
        || left.mode() != right.mode()
        || left.uid() != right.uid()
        || left.gid() != right.gid()
        || left.mtime() != right.mtime()
        || left.mtime_nsec() != right.mtime_nsec()
        || (left.is_file()
            && (left.nlink() != 1 || right.nlink() != 1 || left.len() != right.len()))
        || access_control(from)? != access_control(to)?
    {
        return Err(invalid("account copy metadata differs"));
    }
    let source_attrs = extended_attributes(from)?;
    let mut target_attrs = extended_attributes(to)?;
    if !source_attrs.contains_key(b"com.apple.provenance".as_slice()) {
        target_attrs.remove(b"com.apple.provenance".as_slice());
    }
    if source_attrs != target_attrs {
        return Err(invalid("account copy attributes differ"));
    }
    if left.is_dir() {
        let mut sources = fs::read_dir(from)?.collect::<io::Result<Vec<_>>>()?;
        let mut targets = fs::read_dir(to)?.collect::<io::Result<Vec<_>>>()?;
        sources.sort_by_key(|entry| entry.file_name());
        targets.sort_by_key(|entry| entry.file_name());
        if sources.len() != targets.len() {
            return Err(invalid("account copy paths differ"));
        }
        for (source, target) in sources.into_iter().zip(targets) {
            if source.file_name() != target.file_name() {
                return Err(invalid("account copy paths differ"));
            }
            verify_copy_tree(&source.path(), &target.path())?;
        }
    } else {
        let open = |path: &Path, metadata: &fs::Metadata| -> io::Result<File> {
            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?;
            let opened = file.metadata()?;
            if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
                return Err(invalid("account copy file replaced during proof"));
            }
            Ok(file)
        };
        let mut source = open(from, &left)?;
        let mut target = open(to, &right)?;
        let mut a = [0; 65536];
        let mut b = [0; 65536];
        loop {
            let count = source.read(&mut a)?;
            if count == 0 {
                if target.read(&mut b[..1])? != 0 {
                    return Err(invalid("account copy content differs"));
                }
                break;
            }
            target.read_exact(&mut b[..count])?;
            if a[..count] != b[..count] {
                return Err(invalid("account copy content differs"));
            }
        }
    }
    Ok(())
}
fn sync_tree(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            sync_tree(&entry?.path())?;
        }
        storage::sync_directory(path)
    } else {
        File::open(path)?.sync_all()
    }
}
#[cfg(target_os = "macos")]
pub(crate) fn extended_attributes(path: &Path) -> io::Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    const LIMIT: usize = 16 * 1024 * 1024;
    let path = path_cstring(path)?;
    let count =
        unsafe { libc::listxattr(path.as_ptr(), std::ptr::null_mut(), 0, libc::XATTR_NOFOLLOW) };
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    if count as usize > LIMIT {
        return Err(invalid("extended attributes too large"));
    }
    let mut names = vec![0u8; count as usize];
    let actual = unsafe {
        libc::listxattr(
            path.as_ptr(),
            names.as_mut_ptr().cast(),
            names.len(),
            libc::XATTR_NOFOLLOW,
        )
    };
    if actual < 0 || actual as usize != names.len() {
        return Err(invalid("extended attributes changed during scan"));
    }
    let mut attrs = BTreeMap::new();
    let mut attribute_bytes = names.len();
    for name in names
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let c_name = CString::new(name).map_err(io::Error::other)?;
        let count = unsafe {
            libc::getxattr(
                path.as_ptr(),
                c_name.as_ptr(),
                std::ptr::null_mut(),
                0,
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        attribute_bytes = attribute_bytes
            .checked_add(count as usize)
            .ok_or_else(|| invalid("extended attributes size overflow"))?;
        if attribute_bytes > LIMIT {
            return Err(invalid("extended attributes too large"));
        }
        let mut value = vec![0u8; count as usize];
        let actual = unsafe {
            libc::getxattr(
                path.as_ptr(),
                c_name.as_ptr(),
                value.as_mut_ptr().cast(),
                value.len(),
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        if actual < 0 || actual as usize != value.len() {
            return Err(invalid("extended attribute changed during scan"));
        }
        attrs.insert(name.to_vec(), value);
    }
    #[cfg(test)]
    COPY_PROVENANCE.with(|value| {
        if let Some(map) = value.borrow().as_ref() {
            attrs.remove(b"com.apple.provenance".as_slice());
            let path = Path::new(std::ffi::OsStr::from_bytes(path.as_bytes()));
            let metadata = fs::symlink_metadata(path).unwrap();
            if let Some(attribute) = map.get(&(metadata.dev(), metadata.ino())) {
                attrs.insert(b"com.apple.provenance".to_vec(), attribute.clone());
            }
        }
    });
    Ok(attrs)
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn extended_attributes(_: &Path) -> io::Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "account metadata copying requires macOS",
    ))
}

/// Narrow Darwin ACL FFI. Read-only and deny ACLs are preserved; write-grant ACLs require manual review.
#[cfg(target_os = "macos")]
pub(crate) fn access_control(path: &Path) -> io::Result<Vec<u8>> {
    use std::ffi::c_void;
    unsafe extern "C" {
        fn acl_get_link_np(path: *const libc::c_char, kind: libc::c_int) -> *mut c_void;
        fn acl_free(value: *mut c_void) -> libc::c_int;
        fn acl_valid(acl: *mut c_void) -> libc::c_int;
        fn acl_get_entry(
            acl: *mut c_void,
            index: libc::c_int,
            entry: *mut *mut c_void,
        ) -> libc::c_int;
        fn acl_get_tag_type(entry: *mut c_void, tag: *mut libc::c_int) -> libc::c_int;
        fn acl_get_permset_mask_np(entry: *mut c_void, mask: *mut u64) -> libc::c_int;
        fn acl_to_text(acl: *mut c_void, length: *mut libc::ssize_t) -> *mut libc::c_char;
    }
    struct Allocation(*mut c_void);
    impl Drop for Allocation {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    acl_free(self.0);
                }
            }
        }
    }
    let c_path = path_cstring(path)?;
    let acl = Allocation(unsafe { acl_get_link_np(c_path.as_ptr(), 0x100) }); // ACL_TYPE_EXTENDED, sys/acl.h.
    if acl.0.is_null() {
        let error = io::Error::last_os_error();
        // Darwin reports ENOENT for an existing object without an extended ACL.
        if error.raw_os_error() == Some(libc::ENOENT) {
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.file_type().is_symlink() && (metadata.is_file() || metadata.is_dir()) {
                return Ok(Vec::new());
            }
        }
        return Err(error);
    }
    if unsafe { acl_valid(acl.0) } != 0 {
        return Err(invalid("invalid account ACL"));
    }
    let mut index = 0; // ACL_FIRST_ENTRY. Darwin returns EINVAL at the end of a valid ACL.
    loop {
        let mut entry = std::ptr::null_mut();
        if unsafe { acl_get_entry(acl.0, index, &mut entry) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINVAL) {
                break;
            }
            return Err(error);
        }
        index = -1; // ACL_NEXT_ENTRY.
        let mut tag = 0;
        let mut mask = 0u64;
        if unsafe { acl_get_tag_type(entry, &mut tag) } != 0
            || unsafe { acl_get_permset_mask_np(entry, &mut mask) } != 0
        {
            return Err(io::Error::last_os_error());
        }
        const MUTATING: u64 = (1 << 2)
            | (1 << 4)
            | (1 << 5)
            | (1 << 6)
            | (1 << 8)
            | (1 << 10)
            | (1 << 12)
            | (1 << 13);
        if tag != 1 && tag != 2 {
            return Err(invalid("unsupported account ACL entry"));
        }
        if tag == 1 && mask & MUTATING != 0 {
            return Err(denied(
                "account ACL grants mutation; manual permission review required",
            ));
        }
    }
    let mut length = 0;
    let text = Allocation(unsafe { acl_to_text(acl.0, &mut length) }.cast());
    if text.0.is_null() {
        return Err(io::Error::last_os_error());
    }
    if !(0..=65536).contains(&length) {
        return Err(invalid("account ACL text too large"));
    }
    Ok(unsafe { std::slice::from_raw_parts(text.0.cast::<u8>(), length as usize) }.to_vec())
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn access_control(_: &Path) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "account ACL validation requires macOS",
    ))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    struct Fixture {
        base: PathBuf,
        store: AccountStore,
    }
    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "switcher-account-test-{}-{}",
                std::process::id(),
                NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
            ));
            storage::create_private_directory(&base).unwrap();
            let root = base.join("switcher");
            let active = base.join("Claude-synthetic");
            storage::create_private_directory(&root).unwrap();
            storage::create_private_directory(&active).unwrap();
            storage::atomic_write(&active.join("session"), b"A-original").unwrap();
            let store = AccountStore::new(&root, &active, unsafe { libc::geteuid() }).unwrap();
            Self { base, store }
        }
        fn initialize(&self) {
            let lock = LifecycleLock::try_acquire(&self.store.v2()).unwrap();
            self.store.initialize_checked(&lock, &|| Ok(())).unwrap();
        }
        fn add_b(&self) {
            storage::create_private_directory(&self.store.archive(AccountId::B)).unwrap();
            storage::atomic_write(
                &self.store.archive(AccountId::B).join("session"),
                b"B-private",
            )
            .unwrap();
        }
        fn switch(&self, target: AccountId) -> io::Result<()> {
            let lock = LifecycleLock::try_acquire(&self.store.v2())?;
            self.store
                .switch_checked(target, &lock, &|| Ok(()), &mut |_| Ok(()))
        }
        fn recover(&self) -> io::Result<()> {
            let lock = LifecycleLock::try_acquire(&self.store.v2())?;
            self.store.recover_checked(&lock, &|| Ok(()))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn legacy_selection_recovers_interrupted_exchange_without_backup_permit() {
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        fixture
            .store
            .switch_checked(AccountId::B, &lock, &|| Ok(()), &mut |point| {
                if point == "after_swap" {
                    Err(io::Error::other("synthetic interruption"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        fixture
            .store
            .select_legacy_checked(AccountId::A, &lock, &|| Ok(()))
            .unwrap();
        assert_eq!(
            fs::read(fixture.store.active.join("session")).unwrap(),
            b"A-original"
        );
        assert_eq!(
            fs::read(fixture.store.archive(AccountId::B).join("session")).unwrap(),
            b"B-private"
        );
        assert!(!fixture.store.journal_path().exists());
    }

    #[test]
    fn graceful_quit_requests_once_waits_for_stop_and_bounds_retries() {
        use std::cell::Cell;
        let requests = Cell::new(0);
        let waits = Cell::new(0);
        let running = || {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "synthetic running",
            ))
        };
        assert!(
            quit_claude_with(
                &running,
                &|| {
                    requests.set(requests.get() + 1);
                    Ok(())
                },
                &|| waits.set(waits.get() + 1)
            )
            .is_err()
        );
        assert_eq!(requests.get(), 1);
        assert_eq!(waits.get(), 15);
        let checks = Cell::new(0);
        quit_claude_with(
            &|| {
                checks.set(checks.get() + 1);
                if checks.get() < 3 { running() } else { Ok(()) }
            },
            &|| Ok(()),
            &|| {},
        )
        .unwrap();
        assert_eq!(checks.get(), 3);
        quit_claude_with(
            &|| Ok(()),
            &|| panic!("already stopped must not send AppleScript"),
            &|| {},
        )
        .unwrap();
    }

    #[test]
    fn synthetic_exchange_round_trip_keeps_latest_data_modes_and_xattrs() {
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        storage::atomic_write(&fixture.store.active.join("session"), b"A-latest").unwrap();
        fs::set_permissions(
            fixture.store.active.join("session"),
            fs::Permissions::from_mode(0o640),
        )
        .unwrap();
        let file = path_cstring(&fixture.store.active.join("session")).unwrap();
        let name = CString::new("com.claude-switcher.synthetic").unwrap();
        let value = b"preserve-me";
        assert_eq!(
            unsafe {
                libc::setxattr(
                    file.as_ptr(),
                    name.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                    libc::XATTR_NOFOLLOW,
                )
            },
            0
        );
        let expected = fixture.store.fingerprint(&fixture.store.active).unwrap().0;
        fixture.switch(AccountId::B).unwrap();
        assert_eq!(
            fs::read(fixture.store.active.join("session")).unwrap(),
            b"B-private"
        );
        assert_eq!(
            fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
            b"A-latest"
        );
        assert_eq!(fixture.store.status().unwrap().current, Some(AccountId::B));
        assert_eq!(
            fs::read(fixture.store.root.join("current")).unwrap(),
            b"B\n"
        );
        fixture.switch(AccountId::A).unwrap();
        assert_eq!(
            fixture.store.fingerprint(&fixture.store.active).unwrap().0,
            expected
        );
        assert_eq!(
            fs::metadata(fixture.store.active.join("session"))
                .unwrap()
                .mode()
                & 0o777,
            0o640
        );
        assert!(!fixture.store.status().unwrap().recovery_pending);
    }

    #[test]
    fn all_durable_and_physical_interruption_points_recover_by_identity() {
        let points = [
            "prepared",
            "swap_intent",
            "after_swap",
            "swapped",
            "archive_intent",
            "after_archive",
            "archived",
            "state_committed",
            "mirrored",
            "completed",
            "cleaned",
        ];
        for point in points {
            let fixture = Fixture::new();
            fixture.initialize();
            fixture.add_b();
            storage::atomic_write(&fixture.store.active.join("session"), b"A-latest").unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            let result =
                fixture
                    .store
                    .switch_checked(AccountId::B, &lock, &|| Ok(()), &mut |observed| {
                        if observed == point {
                            Err(io::Error::new(
                                io::ErrorKind::Interrupted,
                                "synthetic interruption",
                            ))
                        } else {
                            Ok(())
                        }
                    });
            assert_eq!(
                result.unwrap_err().kind(),
                io::ErrorKind::Interrupted,
                "{point}"
            );
            drop(lock);
            assert!(fixture.store.status().unwrap().recovery_pending);
            fixture
                .recover()
                .unwrap_or_else(|error| panic!("{point}: {error}"));
            fixture.recover().unwrap();
            let expected = if matches!(point, "prepared" | "swap_intent") {
                AccountId::A
            } else {
                AccountId::B
            };
            assert_eq!(
                fixture.store.status().unwrap().current,
                Some(expected),
                "{point}"
            );
            assert_eq!(
                fs::read(fixture.store.active.join("session")).unwrap(),
                if expected == AccountId::A {
                    &b"A-latest"[..]
                } else {
                    &b"B-private"[..]
                },
                "{point}"
            );
            if expected == AccountId::B {
                assert_eq!(
                    fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
                    b"A-latest",
                    "{point}"
                );
            }
        }
    }

    const PROCESS_CRASH_POINTS: [&str; 11] = [
        "prepared",
        "swap_intent",
        "after_swap",
        "swapped",
        "archive_intent",
        "after_archive",
        "archived",
        "state_committed",
        "mirrored",
        "completed",
        "cleaned",
    ];

    #[test]
    fn account_process_crash_child_fixture() {
        let Some(base) = std::env::var_os("SWITCHER_ACCOUNT_CRASH_BASE") else {
            return;
        };
        let base = PathBuf::from(base);
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(base.parent(), Some(temporary.as_path()));
        assert!(
            base.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("switcher-account-test-")
        );
        storage::validate_no_symlinks(&base).unwrap();
        let metadata = fs::metadata(&base).unwrap();
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.mode() & 0o077, 0);
        let point = std::env::var("SWITCHER_ACCOUNT_CRASH_POINT").unwrap();
        assert!(PROCESS_CRASH_POINTS.contains(&point.as_str()));
        let store = AccountStore::new(
            &base.join("switcher"),
            &base.join("Claude-synthetic"),
            unsafe { libc::geteuid() },
        )
        .unwrap();
        let lock = LifecycleLock::try_acquire(&store.v2()).unwrap();
        store
            .switch_checked(AccountId::B, &lock, &|| Ok(()), &mut |observed| {
                if observed == point {
                    storage::atomic_write(&base.join("crash-ready"), point.as_bytes())?;
                    // Parent kills this unmodified test executable. No application bundle is created.
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
                    while std::time::Instant::now() < deadline {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    panic!("parent did not terminate crash fixture within 30 seconds");
                }
                Ok(())
            })
            .unwrap();
        panic!("requested crash checkpoint was never reached");
    }

    #[test]
    fn killed_process_recovers_every_durable_and_physical_checkpoint() {
        use std::os::unix::process::ExitStatusExt;
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        for point in PROCESS_CRASH_POINTS {
            let fixture = Fixture::new();
            fixture.initialize();
            fixture.add_b();
            storage::atomic_write(&fixture.store.active.join("session"), b"A-latest").unwrap();
            for (directory, value) in [
                (fixture.store.active.clone(), b"A-attribute".as_slice()),
                (
                    fixture.store.archive(AccountId::B),
                    b"B-attribute".as_slice(),
                ),
            ] {
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o750)).unwrap();
                let session = directory.join("session");
                fs::set_permissions(&session, fs::Permissions::from_mode(0o640)).unwrap();
                let file = path_cstring(&session).unwrap();
                let name = CString::new("com.claude-switcher.crash-test").unwrap();
                assert_eq!(
                    unsafe {
                        libc::setxattr(
                            file.as_ptr(),
                            name.as_ptr(),
                            value.as_ptr().cast(),
                            value.len(),
                            0,
                            libc::XATTR_NOFOLLOW,
                        )
                    },
                    0
                );
            }
            let original = identity(&fixture.store.active).unwrap();
            let target_archive = identity(&fixture.store.archive(AccountId::B)).unwrap();
            let a_hash = fixture.store.fingerprint(&fixture.store.active).unwrap().0;
            let b_hash = fixture
                .store
                .fingerprint(&fixture.store.archive(AccountId::B))
                .unwrap()
                .0;
            let a_attrs = extended_attributes(&fixture.store.active.join("session")).unwrap();
            let b_attrs =
                extended_attributes(&fixture.store.archive(AccountId::B).join("session")).unwrap();
            let mut child = ChildGuard(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "accounts::tests::account_process_crash_child_fixture",
                        "--nocapture",
                    ])
                    .env("SWITCHER_ACCOUNT_CRASH_BASE", &fixture.base)
                    .env("SWITCHER_ACCOUNT_CRASH_POINT", point)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let ready = fixture.base.join("crash-ready");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            while !ready.exists() {
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "child exited before {point}"
                );
                assert!(
                    std::time::Instant::now() < deadline,
                    "child timed out before {point}"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert_eq!(fs::read(&ready).unwrap(), point.as_bytes());
            let journal: Journal = storage::read_json(&fixture.store.journal_path()).unwrap();
            assert_eq!(journal.original, original);
            assert_eq!(
                LifecycleLock::try_acquire(&fixture.store.v2())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            child.0.kill().unwrap();
            let status = child.0.wait().unwrap();
            assert_eq!(status.signal(), Some(libc::SIGKILL), "{point}");
            // Reopen after process death: no in-memory account state or lock survives.
            let store = AccountStore::new(
                &fixture.store.root,
                &fixture.store.active,
                fixture.store.owner,
            )
            .unwrap();
            let expected = if matches!(point, "prepared" | "swap_intent") {
                AccountId::A
            } else {
                AccountId::B
            };
            for _ in 0..2 {
                let lock = LifecycleLock::try_acquire(&store.v2()).unwrap();
                store
                    .recover_checked(&lock, &|| Ok(()))
                    .unwrap_or_else(|error| panic!("{point}: {error}"));
                let state = store.state().unwrap().unwrap();
                assert_eq!(state.account, expected, "{point}");
                assert_eq!(state.directory, identity(&store.active).unwrap(), "{point}");
                assert_eq!(
                    state.generation,
                    if expected == AccountId::A { 1 } else { 2 }
                );
                assert_eq!(
                    identity(&store.active).unwrap(),
                    if expected == AccountId::A {
                        original
                    } else {
                        journal.staged
                    }
                );
                assert_eq!(
                    store.fingerprint(&store.active).unwrap().0,
                    if expected == AccountId::A {
                        &a_hash
                    } else {
                        &b_hash
                    }
                    .as_str()
                );
                assert_eq!(
                    fs::read(store.active.join("session")).unwrap(),
                    if expected == AccountId::A {
                        b"A-latest".as_slice()
                    } else {
                        b"B-private".as_slice()
                    }
                );
                assert_eq!(
                    extended_attributes(&store.active.join("session")).unwrap(),
                    if expected == AccountId::A {
                        a_attrs.clone()
                    } else {
                        b_attrs.clone()
                    }
                );
                assert_eq!(fs::metadata(&store.active).unwrap().mode() & 0o777, 0o750);
                assert_eq!(
                    fs::metadata(store.active.join("session")).unwrap().mode() & 0o777,
                    0o640
                );
                assert_eq!(
                    identity(&store.archive(AccountId::B)).unwrap(),
                    target_archive
                );
                assert_eq!(
                    store.fingerprint(&store.archive(AccountId::B)).unwrap().0,
                    b_hash
                );
                if expected == AccountId::B {
                    assert_eq!(identity(&store.archive(AccountId::A)).unwrap(), original);
                    assert_eq!(
                        store.fingerprint(&store.archive(AccountId::A)).unwrap().0,
                        a_hash
                    );
                    assert_eq!(
                        extended_attributes(&store.archive(AccountId::A).join("session")).unwrap(),
                        a_attrs
                    );
                }
                assert_eq!(
                    fs::read(store.root.join("current")).unwrap(),
                    if expected == AccountId::A {
                        b"A\n".as_slice()
                    } else {
                        b"B\n".as_slice()
                    }
                );
                assert!(!store.journal_path().exists());
                assert_eq!(fs::read_dir(store.transactions()).unwrap().count(), 0);
            }
            println!(
                "checkpoint={point} signal=SIGKILL recoveries=2 current={} identity/content/mode/xattr/mirror/journal=ok",
                expected.name()
            );
        }
    }

    #[test]
    fn existing_a_missing_state_requires_matching_explicit_preview() {
        let fixture = Fixture::new();
        fixture.initialize();
        fs::remove_file(fixture.store.state_path()).unwrap();
        fs::remove_file(fixture.store.root.join("current")).unwrap();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        assert!(fixture.store.initialize_checked(&lock, &|| Ok(())).is_err());
        let preview = fixture.store.preview_association_a().unwrap();
        fixture
            .store
            .associate_checked(&lock, &preview, &|| Ok(()))
            .unwrap();
        assert_eq!(fixture.store.status().unwrap().current, Some(AccountId::A));
        fs::remove_file(fixture.store.state_path()).unwrap();
        storage::atomic_write(&fixture.store.active.join("session"), b"changed").unwrap();
        assert!(
            fixture
                .store
                .associate_checked(&lock, &preview, &|| Ok(()))
                .is_err()
        );
        assert!(!fixture.store.state_path().try_exists().unwrap());
    }

    #[test]
    fn malformed_journal_and_replaced_directory_preserve_every_directory() {
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        storage::atomic_write(&fixture.store.journal_path(), b"{broken").unwrap();
        let original = identity(&fixture.store.active).unwrap();
        assert!(fixture.recover().is_err());
        assert_eq!(identity(&fixture.store.active).unwrap(), original);
        fs::remove_file(fixture.store.journal_path()).unwrap();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        fixture
            .store
            .switch_checked(AccountId::B, &lock, &|| Ok(()), &mut |point| {
                if point == "after_swap" {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "stop"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        drop(lock);
        let displaced = fixture.base.join("displaced");
        fs::rename(&fixture.store.active, &displaced).unwrap();
        storage::create_private_directory(&fixture.store.active).unwrap();
        let fresh = identity(&fixture.store.active).unwrap();
        assert!(fixture.recover().is_err());
        assert_eq!(identity(&fixture.store.active).unwrap(), fresh);
        assert!(displaced.is_dir());
        assert!(fixture.store.journal_path().is_file());
    }

    #[test]
    fn unsafe_links_owners_space_and_lock_conflicts_fail_before_exchange() {
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        assert!(
            AccountStore::new(
                &fixture.store.root,
                &fixture.store.active,
                unsafe { libc::geteuid() }.saturating_add(1)
            )
            .is_err()
        );
        assert_eq!(
            ensure_space(&fixture.store.v2(), u64::MAX)
                .unwrap_err()
                .kind(),
            io::ErrorKind::StorageFull
        );
        let link = fixture.store.archive(AccountId::B).join("unsafe");
        std::os::unix::fs::symlink(&fixture.store.active, &link).unwrap();
        assert!(fixture.switch(AccountId::B).is_err());
        fs::remove_file(&link).unwrap();
        fs::hard_link(fixture.store.active.join("session"), &link).unwrap();
        assert!(fixture.switch(AccountId::B).is_err());
        fs::remove_file(&link).unwrap();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        assert_eq!(
            fixture.store.switch_to(AccountId::B).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let result = fixture.store.switch_checked(
            AccountId::B,
            &lock,
            &|| {
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "synthetic Claude running",
                ))
            },
            &mut |_| Ok(()),
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            fs::read(fixture.store.active.join("session")).unwrap(),
            b"A-original"
        );
    }

    #[test]
    fn derived_current_is_repaired_from_authoritative_state() {
        let fixture = Fixture::new();
        fixture.initialize();
        storage::atomic_write(&fixture.store.root.join("current"), b"B\n").unwrap();
        fixture.recover().unwrap();
        assert_eq!(
            fs::read(fixture.store.root.join("current")).unwrap(),
            b"A\n"
        );
    }
    #[test]
    fn copy_provenance_addition_is_allowed_only_for_a_new_verified_copy() {
        let _simulation = CopyProvenanceFixture::new();
        let fixture = Fixture::new();
        let destination = fixture.base.join("copy");
        fixture
            .store
            .copy_verified(&fixture.store.active, &destination)
            .unwrap();
        assert_ne!(
            fixture.store.fingerprint(&fixture.store.active).unwrap().0,
            fixture.store.fingerprint(&destination).unwrap().0
        );
    }

    #[test]
    fn copy_provenance_direction_and_existing_values_remain_strict() {
        let _simulation = CopyProvenanceFixture::new();
        let fixture = Fixture::new();
        let destination = fixture.base.join("copy");
        fixture
            .store
            .copy_verified(&fixture.store.active, &destination)
            .unwrap();
        assert!(verify_copy_tree(&fixture.store.active, &destination).is_ok());
        assert!(verify_copy_tree(&destination, &fixture.store.active).is_err());
        CopyProvenanceFixture::set(&fixture.store.active, Some(b"source"));
        assert!(verify_copy_tree(&fixture.store.active, &destination).is_err());
        CopyProvenanceFixture::set(&destination, Some(b"source"));
        assert!(verify_copy_tree(&fixture.store.active, &destination).is_ok());
        CopyProvenanceFixture::set(&destination, Some(b"changed"));
        assert!(verify_copy_tree(&fixture.store.active, &destination).is_err());
        CopyProvenanceFixture::set(&destination, Some(b"source"));
        fs::set_permissions(
            destination.join("session"),
            fs::Permissions::from_mode(0o640),
        )
        .unwrap();
        assert!(verify_copy_tree(&fixture.store.active, &destination).is_err());
    }
    fn restore_copy_times(path: &Path, before: &fs::Metadata) -> io::Result<()> {
        let times = [
            libc::timespec {
                tv_sec: before.atime(),
                tv_nsec: before.atime_nsec(),
            },
            libc::timespec {
                tv_sec: before.mtime(),
                tv_nsec: before.mtime_nsec(),
            },
        ];
        if unsafe {
            libc::utimensat(
                libc::AT_FDCWD,
                path_cstring(path)?.as_ptr(),
                times.as_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    fn assert_copy_rejects_post_proof_mutation(mutate: CopyAfterProof) {
        let fixture = Fixture::new();
        let source_hash = fixture.store.fingerprint(&fixture.store.active).unwrap();
        let source_metadata = copy_metadata(&fixture.store.active).unwrap();
        let destination = fixture.base.join("copy");
        COPY_AFTER_PROOF.with(|hook| {
            assert!(hook.replace(Some(mutate)).is_none());
        });
        let result = fixture
            .store
            .copy_verified(&fixture.store.active, &destination);
        assert!(COPY_AFTER_PROOF.with(|hook| hook.get().is_none()));
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert!(
            destination.exists(),
            "failed copy remains available for inspection"
        );
        assert_eq!(
            fixture.store.fingerprint(&fixture.store.active).unwrap(),
            source_hash
        );
        assert_eq!(
            copy_metadata(&fixture.store.active).unwrap(),
            source_metadata
        );
    }
    #[test]
    fn copy_rejects_destination_content_changed_after_proof() {
        assert_copy_rejects_post_proof_mutation(|destination| {
            let target = destination.join("session");
            let before = fs::symlink_metadata(&target)?;
            fs::write(&target, b"B-original")?;
            restore_copy_times(&target, &before)
        });
    }
    #[test]
    fn copy_rejects_destination_mtime_changed_after_proof() {
        assert_copy_rejects_post_proof_mutation(|destination| {
            let target = destination.join("session");
            let before = fs::symlink_metadata(&target)?;
            let times = [
                libc::timespec {
                    tv_sec: before.atime(),
                    tv_nsec: before.atime_nsec(),
                },
                libc::timespec {
                    tv_sec: before.mtime() + 1,
                    tv_nsec: before.mtime_nsec(),
                },
            ];
            if unsafe {
                libc::utimensat(
                    libc::AT_FDCWD,
                    path_cstring(&target)?.as_ptr(),
                    times.as_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    #[test]
    fn copy_rejects_destination_child_replaced_after_proof() {
        assert_copy_rejects_post_proof_mutation(|destination| {
            let root_before = fs::symlink_metadata(destination)?;
            let target = destination.join("session");
            let before = fs::symlink_metadata(&target)?;
            let replacement = destination.join("replacement");
            let status = Command::new("/usr/bin/ditto")
                .args(["--rsrc", "--extattr", "--acl"])
                .arg(&target)
                .arg(&replacement)
                .env_clear()
                .env("LC_ALL", "C")
                .status()?;
            if !status.success() {
                return Err(io::Error::other("synthetic replacement copy failed"));
            }
            fs::rename(replacement, &target)?;
            assert_ne!(fs::symlink_metadata(&target)?.ino(), before.ino());
            restore_copy_times(&target, &before)?;
            restore_copy_times(destination, &root_before)
        });
    }

    #[test]
    fn copy_proof_rejects_other_xattrs_acl_content_and_mtime_changes() {
        for mutation in ["xattr", "acl", "content", "mtime"] {
            let fixture = Fixture::new();
            let destination = fixture.base.join("copy");
            fixture
                .store
                .copy_verified(&fixture.store.active, &destination)
                .unwrap();
            let source = fixture.store.active.join("session");
            let target = destination.join("session");
            let before = fs::symlink_metadata(&source).unwrap();
            match mutation {
                "xattr" => {
                    let path = path_cstring(&target).unwrap();
                    let name = CString::new("com.claude-switcher.copy-proof").unwrap();
                    assert_eq!(
                        unsafe {
                            libc::setxattr(
                                path.as_ptr(),
                                name.as_ptr(),
                                b"x".as_ptr().cast(),
                                1,
                                0,
                                libc::XATTR_NOFOLLOW,
                            )
                        },
                        0
                    );
                }
                "acl" => {
                    assert!(
                        Command::new("/bin/chmod")
                            .args(["+a", "everyone allow read"])
                            .arg(&target)
                            .status()
                            .unwrap()
                            .success()
                    );
                }
                "content" => {
                    fs::write(&target, b"B-original").unwrap();
                    let times = [
                        libc::timespec {
                            tv_sec: before.atime(),
                            tv_nsec: before.atime_nsec(),
                        },
                        libc::timespec {
                            tv_sec: before.mtime(),
                            tv_nsec: before.mtime_nsec(),
                        },
                    ];
                    assert_eq!(
                        unsafe {
                            libc::utimensat(
                                libc::AT_FDCWD,
                                path_cstring(&target).unwrap().as_ptr(),
                                times.as_ptr(),
                                libc::AT_SYMLINK_NOFOLLOW,
                            )
                        },
                        0
                    );
                }
                "mtime" => {
                    let snapshot = copy_metadata(&fixture.store.active).unwrap();
                    let times = [
                        libc::timespec {
                            tv_sec: before.atime(),
                            tv_nsec: before.atime_nsec(),
                        },
                        libc::timespec {
                            tv_sec: before.mtime() + 1,
                            tv_nsec: before.mtime_nsec(),
                        },
                    ];
                    assert_eq!(
                        unsafe {
                            libc::utimensat(
                                libc::AT_FDCWD,
                                path_cstring(&target).unwrap().as_ptr(),
                                times.as_ptr(),
                                libc::AT_SYMLINK_NOFOLLOW,
                            )
                        },
                        0
                    );
                    assert_eq!(copy_metadata(&fixture.store.active).unwrap(), snapshot);
                }
                _ => unreachable!(),
            }
            assert!(
                verify_copy_tree(&fixture.store.active, &destination).is_err(),
                "{mutation}"
            );
        }
    }
    #[test]
    fn copy_provenance_round_trip_and_separate_artifact_tampering() {
        let _simulation = CopyProvenanceFixture::new();
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        fixture.switch(AccountId::B).unwrap();
        fixture.switch(AccountId::A).unwrap();
        assert_eq!(
            fs::read(fixture.store.active.join("session")).unwrap(),
            b"A-original"
        );
        for artifact in ["incoming", "archive"] {
            let fixture = Fixture::new();
            fixture.initialize();
            fixture.add_b();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            assert!(
                fixture
                    .store
                    .switch_checked(AccountId::B, &lock, &|| Ok(()), &mut |phase| {
                        if phase == "prepared" {
                            Err(io::Error::other("interrupted"))
                        } else {
                            Ok(())
                        }
                    })
                    .is_err()
            );
            let journal: Journal = storage::read_json(&fixture.store.journal_path()).unwrap();
            assert_ne!(journal.target_hash, journal.incoming_hash());
            let path = if artifact == "incoming" {
                fixture
                    .store
                    .transactions()
                    .join(&journal.transaction)
                    .join("incoming")
            } else {
                fixture.store.archive(AccountId::B)
            };
            CopyProvenanceFixture::set(&path, Some(b"tampered"));
            drop(lock);
            assert!(fixture.recover().is_err(), "{artifact}");
        }
    }
    #[test]
    fn copy_provenance_exchange_recovers_all_existing_checkpoints() {
        let _simulation = CopyProvenanceFixture::new();
        all_durable_and_physical_interruption_points_recover_by_identity();
    }
    #[test]
    fn journal_staged_hash_legacy_fallback_and_format_validation() {
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        assert!(
            fixture
                .store
                .switch_checked(AccountId::B, &lock, &|| Ok(()), &mut |phase| {
                    if phase == "prepared" {
                        Err(io::Error::other("interrupted"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        let mut value: serde_json::Value =
            storage::read_json(&fixture.store.journal_path()).unwrap();
        value.as_object_mut().unwrap().remove("staged_hash");
        let legacy: Journal = serde_json::from_value(value.clone()).unwrap();
        fixture.store.validate_journal(&legacy).unwrap();
        assert_eq!(legacy.incoming_hash(), legacy.target_hash);
        for bad in [
            serde_json::Value::Null,
            serde_json::json!(""),
            serde_json::json!("not-a-hash"),
        ] {
            value["staged_hash"] = bad;
            if let Ok(journal) = serde_json::from_value::<Journal>(value.clone()) {
                assert!(fixture.store.validate_journal(&journal).is_err());
            }
        }
        storage::atomic_json(&fixture.store.journal_path(), &legacy).unwrap();
        drop(lock);
        fixture.recover().unwrap();
    }

    #[test]
    fn legacy_0755_archive_container_opens_without_changing_permissions() {
        let fixture = Fixture::new();
        let store = &fixture.store;
        fs::set_permissions(&store.root, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(store.archives(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(fs::read_dir(store.archives()).unwrap().count(), 0);
        let archives_before = identity(&store.archives()).unwrap();
        let active_before = identity(&store.active).unwrap();
        let result = AccountStore::new(&store.root, &store.active, store.owner);
        assert_eq!(identity(&store.archives()).unwrap(), archives_before);
        assert_eq!(identity(&store.active).unwrap(), active_before);
        for (path, mode) in [
            (store.root.clone(), 0o755),
            (store.archives(), 0o755),
            (store.active.clone(), 0o700),
            (store.v2(), 0o700),
        ] {
            assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, mode);
        }
        assert_eq!(
            fs::read(store.active.join("session")).unwrap(),
            b"A-original"
        );
        result.expect("safe legacy 0755 archive containers must remain usable");
    }

    #[test]
    fn legacy_archive_contents_and_metadata_survive_open() {
        let fixture = Fixture::new();
        let store = &fixture.store;
        fs::set_permissions(store.archives(), fs::Permissions::from_mode(0o755)).unwrap();
        storage::create_private_directory(&store.archive(AccountId::A)).unwrap();
        storage::atomic_write(&store.archive(AccountId::A).join("session"), b"legacy-A").unwrap();
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow read"])
                .arg(store.archives())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("/usr/bin/xattr")
                .args(["-w", "com.claude-switcher.test", "synthetic"])
                .arg(store.archives())
                .status()
                .unwrap()
                .success()
        );
        let before = store.fingerprint(&store.archives()).unwrap();
        let metadata = fs::metadata(store.archives()).unwrap();
        let original = identity(&store.archives()).unwrap();
        AccountStore::new(&store.root, &store.active, store.owner).unwrap();
        assert_eq!(store.fingerprint(&store.archives()).unwrap(), before);
        assert_eq!(identity(&store.archives()).unwrap(), original);
        let after = fs::metadata(store.archives()).unwrap();
        assert_eq!(
            (
                after.uid(),
                after.gid(),
                after.mode(),
                after.mtime(),
                after.mtime_nsec()
            ),
            (
                metadata.uid(),
                metadata.gid(),
                metadata.mode(),
                metadata.mtime(),
                metadata.mtime_nsec()
            )
        );
    }

    #[test]
    fn missing_archive_container_is_created_private() {
        let fixture = Fixture::new();
        let store = &fixture.store;
        fs::remove_dir(store.archives()).unwrap();
        AccountStore::new(&store.root, &store.active, store.owner).unwrap();
        assert_eq!(
            fs::metadata(store.archives()).unwrap().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn legacy_archive_container_modes_and_acl_are_accepted_but_root_links_are_not() {
        for mode in [0o775, 0o757, 0o777] {
            let fixture = Fixture::new();
            let store = &fixture.store;
            fs::set_permissions(store.archives(), fs::Permissions::from_mode(mode)).unwrap();
            AccountStore::new(&store.root, &store.active, store.owner).unwrap();
            assert_eq!(fs::metadata(store.archives()).unwrap().mode() & 0o777, mode);
        }
        for dangling in [false, true] {
            let fixture = Fixture::new();
            let store = &fixture.store;
            fs::remove_dir(store.archives()).unwrap();
            let target = if dangling {
                fixture.base.join("missing")
            } else {
                store.active.clone()
            };
            std::os::unix::fs::symlink(target, store.archives()).unwrap();
            assert_eq!(
                AccountStore::new(&store.root, &store.active, store.owner)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
        }
        let fixture = Fixture::new();
        let store = &fixture.store;
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow write"])
                .arg(store.archives())
                .status()
                .unwrap()
                .success()
        );
        AccountStore::new(&store.root, &store.active, store.owner).unwrap();
    }

    #[test]
    fn legacy_archive_compatibility_keeps_v2_directories_private() {
        for relative in ["v2", "v2/transactions"] {
            let fixture = Fixture::new();
            let store = &fixture.store;
            fs::set_permissions(store.archives(), fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(store.root.join(relative), fs::Permissions::from_mode(0o755))
                .unwrap();
            assert_eq!(
                AccountStore::new(&store.root, &store.active, store.owner)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
            assert_eq!(
                fs::metadata(store.root.join(relative)).unwrap().mode() & 0o777,
                0o755
            );
        }
    }

    #[test]
    fn readonly_inspection_uses_legacy_current_without_creating_v2() {
        let fixture = Fixture::new();
        fs::remove_dir_all(fixture.store.v2()).unwrap();
        storage::atomic_write(&fixture.store.root.join("current"), b"B\n").unwrap();
        let status = AccountStore::inspect(
            &fixture.store.root,
            &fixture.store.active,
            fixture.store.owner,
        )
        .unwrap();
        assert_eq!(status.current, Some(AccountId::B));
        assert!(!fixture.store.v2().exists());
        storage::create_private_directory(&fixture.store.archive(AccountId::A)).unwrap();
        assert!(
            AccountStore::inspect(
                &fixture.store.root,
                &fixture.store.active,
                fixture.store.owner
            )
            .unwrap()
            .a_saved
        );
        let absent = fixture.base.join("absent");
        assert_eq!(
            AccountStore::inspect(&absent, &fixture.store.active, fixture.store.owner)
                .unwrap()
                .current,
            Some(AccountId::A)
        );
        assert!(!absent.exists());
    }

    #[test]
    fn synthetic_process_records_block_account_initialization_without_running_apps() {
        let fixture = Fixture::new();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        for command in [
            "Claude",
            "/Applications/Claude.app/Contents/MacOS/Claude",
            "/synthetic path/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper",
        ] {
            let listing = format!("  {} {command}\n", fixture.store.owner);
            let result = fixture.store.initialize_checked(&lock, &|| {
                ensure_claude_stopped_listing(fixture.store.owner, listing.as_bytes())
            });
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
            assert!(!fixture.store.state_path().exists());
            assert!(!fixture.store.archive(AccountId::A).exists());
            assert_eq!(
                fs::read_dir(fixture.store.transactions()).unwrap().count(),
                0
            );
        }
    }
    #[test]
    fn synthetic_process_records_preserve_owner_and_path_boundaries() {
        let owner = 501;
        ensure_claude_stopped_listing(
            owner,
            b"  502 /Applications/Claude.app/Contents/MacOS/Claude\n 501 /bin/sleep\n 501 /Applications/OtherClaude.app/Contents/MacOS/OtherClaude\n 501 claude\n",
        )
        .unwrap();
        for listing in [
            b"missing-owner".as_slice(),
            b"not-a-uid /bin/sleep\n".as_slice(),
            b"501 \xff\n".as_slice(),
        ] {
            assert_eq!(
                ensure_claude_stopped_listing(owner, listing)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
    #[test]
    fn readonly_acl_is_preserved_and_write_grant_acl_is_rejected() {
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        let file = fixture.store.archive(AccountId::B).join("session");
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow read"])
                .arg(&file)
                .status()
                .unwrap()
                .success()
        );
        let expected = access_control(&file).unwrap();
        fixture.switch(AccountId::B).unwrap();
        assert_eq!(
            access_control(&fixture.store.active.join("session")).unwrap(),
            expected
        );
        fixture.switch(AccountId::A).unwrap();
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow write"])
                .arg(&file)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            fixture.switch(AccountId::B).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fixture.store.status().unwrap().current, Some(AccountId::A));
    }
    #[test]
    fn missing_outgoing_archive_is_recreated_and_recovered_after_rename() {
        let fixture = Fixture::new();
        fixture.initialize();
        fixture.add_b();
        fs::remove_dir_all(fixture.store.archive(AccountId::A)).unwrap();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        fixture
            .store
            .switch_checked(AccountId::B, &lock, &|| Ok(()), &mut |point| {
                if point == "after_archive" {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "stop"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        drop(lock);
        fixture.recover().unwrap();
        assert_eq!(fixture.store.status().unwrap().current, Some(AccountId::B));
        assert_eq!(
            fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
            b"A-original"
        );
    }
    #[test]
    fn pending_repairs_survive_lock_release_and_block_all_account_mutations() {
        for relative in ["v2/region-repair.json", "v2/user-repairs/journal.json"] {
            let fixture = Fixture::new();
            fixture.initialize();
            let store = &fixture.store;
            let target = store.root.join(relative);
            storage::create_private_directory(target.parent().unwrap()).unwrap();
            storage::atomic_write(&target, b"synthetic durable repair pending").unwrap();
            let active_before = fs::read(store.active.join("session")).unwrap();
            let state_before = fs::read(store.state_path()).unwrap();
            // There is no held repair lock: the journal alone must block a fresh operation.
            assert!(
                fixture
                    .switch(AccountId::B)
                    .unwrap_err()
                    .to_string()
                    .contains("先恢复修复事务")
            );
            assert!(fixture.recover().is_err());
            assert!(AccountStore::new(&store.root, &store.active, store.owner).is_err());
            assert_eq!(
                fs::read(store.active.join("session")).unwrap(),
                active_before
            );
            assert_eq!(fs::read(store.state_path()).unwrap(), state_before);
            fs::remove_file(&target).unwrap();
            assert!(ensure_no_pending_repairs(&store.root).is_ok());
            // A dangling journal symlink must also remain fail-closed.
            std::os::unix::fs::symlink(store.root.join("missing"), &target).unwrap();
            assert!(ensure_no_pending_repairs(&store.root).is_err());
        }
    }
}
