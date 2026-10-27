//! Ditto-compatible account transactions. Staging follows each destination volume.
use super::*;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyJournal {
    schema: u32,
    #[serde(default)]
    in_place: bool,
    // None identifies old journals, whose empty source cannot prove archive ownership.
    #[serde(default)]
    preserve_archive: Option<bool>,
    transaction: String,
    before: ActiveState,
    target: AccountId,
    active_stage: Identity,
    archive_stage: Identity,
    original: Identity,
    incoming: Identity,
    outgoing: Identity,
    previous_archive: Option<Identity>,
    original_hash: String,
    incoming_hash: String,
    outgoing_hash: String,
}
fn journal_path(store: &AccountStore) -> PathBuf {
    store.v2().join("legacy-account-journal.json")
}
fn stages(store: &AccountStore, transaction: &str) -> (PathBuf, PathBuf) {
    (
        store
            .active
            .parent()
            .unwrap()
            .join(format!(".claude-switcher-{transaction}")),
        store.archives().join(format!(".{transaction}")),
    )
}
fn validate(store: &AccountStore, journal: &LegacyJournal) -> io::Result<()> {
    if journal.schema != 1
        || !journal.transaction.starts_with("tx-")
        || journal.transaction.len() > 80
        || !journal
            .transaction
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || journal.before.schema != 1
        || journal.before.generation == 0
        || journal.before.directory != journal.original
        || journal.outgoing_hash != journal.original_hash
        || (journal.in_place
            && (journal.target != journal.before.account
                || journal.incoming != journal.original
                || journal.incoming_hash != journal.original_hash))
        || (journal.preserve_archive == Some(true)
            && (journal.in_place
                || journal.before.account != AccountId::A
                || journal.original_hash != empty_directory_fingerprint()))
        || !valid_hash(&journal.original_hash)
        || !valid_hash(&journal.incoming_hash)
        || !valid_hash(&journal.outgoing_hash)
    {
        return Err(invalid("invalid legacy transaction"));
    }
    // Old transactions did not distinguish a default A from a recorded owner.
    // Stop before any exchange or cleanup, including after an interrupted commit.
    if journal.preserve_archive.is_none()
        && journal.before.account == AccountId::A
        && journal.previous_archive.is_some()
        && journal.original_hash == empty_directory_fingerprint()
    {
        return Err(invalid(
            "legacy empty-source archive transaction needs manual recovery; data preserved",
        ));
    }
    let (active_stage, archive_stage) = stages(store, &journal.transaction);
    if optional_identity(&active_stage)?.is_some_and(|id| id != journal.active_stage)
        || optional_identity(&archive_stage)?.is_some_and(|id| id != journal.archive_stage)
    {
        return Err(invalid("legacy staging identity changed"));
    }
    Ok(())
}
fn check_lock(store: &AccountStore, lock: &LifecycleLock) -> io::Result<()> {
    if !lock.protects(&store.v2()) {
        return Err(denied("account lock changed"));
    }
    store.check_directory(&store.root)?;
    storage::validate_no_symlinks(&store.active)?;
    storage::validate_no_symlinks(&store.archives())?;
    identity(&store.active)?;
    identity(&store.archives())?;
    Ok(())
}

// Before the journal owns recovery, discard only directories created by this attempt.
struct PendingStages {
    paths: Vec<PathBuf>,
    journal: PathBuf,
    journal_attempted: bool,
    handed_off: bool,
}
impl PendingStages {
    fn create(&mut self, path: &Path) -> io::Result<()> {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)?;
        self.paths.push(path.to_owned());
        Ok(())
    }
}
impl Drop for PendingStages {
    fn drop(&mut self) {
        if self.handed_off {
            return;
        }
        if self.journal_attempted {
            // atomic_json can fail after rename (parent fsync). Keep recovery data
            // unless absence is certain; even a metadata error must not delete it.
            match fs::symlink_metadata(&self.journal) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                _ => return,
            }
        }
        for path in self.paths.iter().rev() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

pub(super) fn select(
    store: &AccountStore,
    target: AccountId,
    lock: &LifecycleLock,
    stopped: &dyn Fn() -> io::Result<()>,
) -> io::Result<()> {
    select_with(store, target, lock, stopped, &mut |_| Ok(()))
}
fn select_with(
    store: &AccountStore,
    target: AccountId,
    lock: &LifecycleLock,
    stopped: &dyn Fn() -> io::Result<()>,
    hook: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<()> {
    check_lock(store, lock)?;
    stopped()?;
    if store.journal_path().try_exists()? {
        store.recover_checked(lock, stopped)?;
    }
    recover(store, lock, stopped)?;
    let recorded = store.recorded_account()?;
    let current = recorded.unwrap_or(AccountId::A);
    let first = recorded.is_none();
    if target == current && !first && store.archive(current).exists() {
        store.mirror(current)?;
        return Ok(());
    }
    let transaction = format!(
        "tx-{}-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos(),
        std::process::id(),
        NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
    );
    let (active_stage, archive_stage) = stages(store, &transaction);
    let mut pending = PendingStages {
        paths: Vec::new(),
        journal: journal_path(store),
        journal_attempted: false,
        handed_off: false,
    };
    pending.create(&active_stage)?;
    pending.create(&archive_stage)?;
    hook("stages_created")?;
    let incoming = active_stage.join("incoming");
    let outgoing = archive_stage.join("outgoing");
    let original = identity(&store.active)?;
    let original_hash = fingerprint(&store.active)?;
    let preserve_archive = first && original_hash == empty_directory_fingerprint();
    hook("source_snapshotted")?;
    copy(&store.active, &outgoing)?;
    hook("outgoing_copied")?;
    // First A adoption and same-account archive repair retain the live directory.
    let in_place = !preserve_archive && target == current;
    if in_place {
        // The outgoing copy is the archive; no replacement live copy is needed.
    } else if store.archive(target).exists() {
        copy(&store.archive(target), &incoming)?;
    } else {
        storage::create_private_directory(&incoming)?;
    }
    hook("incoming_copied")?;
    stopped()?;
    if identity(&store.active)? != original || fingerprint(&store.active)? != original_hash {
        return Err(invalid("live account changed while copying"));
    }
    let before = ActiveState {
        schema: 1,
        account: current,
        generation: store.state()?.map(|state| state.generation).unwrap_or(1),
        directory: original,
        transaction: None,
    };
    let journal = LegacyJournal {
        schema: 1,
        in_place,
        preserve_archive: Some(preserve_archive),
        transaction,
        before,
        target,
        active_stage: identity(&active_stage)?,
        archive_stage: identity(&archive_stage)?,
        original,
        incoming: if in_place {
            original
        } else {
            identity(&incoming)?
        },
        outgoing: identity(&outgoing)?,
        previous_archive: optional_identity(&store.archive(current))?,
        original_hash,
        incoming_hash: if in_place {
            fingerprint(&store.active)?
        } else {
            fingerprint(&incoming)?
        },
        outgoing_hash: fingerprint(&outgoing)?,
    };
    validate(store, &journal)?;
    hook("before_journal")?;
    pending.journal_attempted = true;
    storage::atomic_json(&journal_path(store), &journal)?;
    // This injection point also models a parent fsync error after journal rename.
    hook("journal_written")?;
    pending.handed_off = true;
    hook("prepared")?;
    finish(store, &journal, lock, stopped, hook)
}
pub(super) fn recover(
    store: &AccountStore,
    lock: &LifecycleLock,
    stopped: &dyn Fn() -> io::Result<()>,
) -> io::Result<()> {
    let journal: LegacyJournal = match storage::read_json(&journal_path(store)) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    finish(store, &journal, lock, stopped, &mut |_| Ok(()))
}
fn finish(
    store: &AccountStore,
    journal: &LegacyJournal,
    lock: &LifecycleLock,
    stopped: &dyn Fn() -> io::Result<()>,
    hook: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<()> {
    check_lock(store, lock)?;
    validate(store, journal)?;
    stopped()?;
    let (active_stage, archive_stage) = stages(store, &journal.transaction);
    let incoming = active_stage.join("incoming");
    let outgoing = archive_stage.join("outgoing");
    let archive = store.archive(journal.before.account);
    let expected = ActiveState {
        schema: 1,
        account: journal.target,
        generation: journal
            .before
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("account generation overflow"))?,
        directory: journal.incoming,
        transaction: Some(journal.transaction.clone()),
    };
    // A committed transaction may have already removed one or both staging directories.
    let committed = store.state()?.as_ref() == Some(&expected);
    if !journal.in_place && identity(&store.active)? == journal.original {
        if committed
            || identity(&incoming)? != journal.incoming
            || fingerprint(&store.active)? != journal.original_hash
            || fingerprint(&incoming)? != journal.incoming_hash
        {
            return Err(invalid("prepared account data changed"));
        }
        exchange(&store.active, &incoming)?;
        hook("active_exchanged")?;
        storage::sync_directory(store.active.parent().unwrap())?;
        storage::sync_directory(&active_stage)?;
        hook("swapped")?;
    }
    if identity(&store.active)? != journal.incoming
        || fingerprint(&store.active)? != journal.incoming_hash
    {
        return Err(invalid("selected account changed during recovery"));
    }
    if journal.preserve_archive == Some(true) {
        if optional_identity(&archive)? != journal.previous_archive {
            return Err(invalid("preserved account archive changed"));
        }
        match optional_identity(&outgoing)? {
            Some(id)
                if id != journal.outgoing
                    || fingerprint(&outgoing)? != journal.outgoing_hash
                    || fs::read_dir(&outgoing)?.next().transpose()?.is_some() =>
            {
                return Err(invalid("unowned empty account staging changed"));
            }
            None if !committed => return Err(invalid("unowned empty account staging missing")),
            _ => {}
        }
        hook("archived")?;
    } else {
        if optional_identity(&archive)? != Some(journal.outgoing) {
            if committed
                || optional_identity(&archive)? != journal.previous_archive
                || identity(&outgoing)? != journal.outgoing
                || fingerprint(&outgoing)? != journal.outgoing_hash
            {
                return Err(invalid("outgoing account archive changed"));
            }
            if journal.previous_archive.is_some() {
                exchange(&outgoing, &archive)?;
            } else {
                fs::rename(&outgoing, &archive)?;
            }
            storage::sync_directory(&store.archives())?;
            storage::sync_directory(&archive_stage)?;
            hook("archived")?;
        }
        if fingerprint(&archive)? != journal.outgoing_hash {
            return Err(invalid("outgoing account content changed"));
        }
    }
    stopped()?;
    storage::atomic_json(&store.state_path(), &expected)?;
    hook("committed")?;
    store.mirror(journal.target)?;
    hook("mirrored")?;
    // remove_dir_all unlinks symbolic links; it never follows their targets.
    clean_stage(
        &active_stage,
        journal.active_stage,
        "incoming",
        if journal.in_place {
            None
        } else {
            Some(journal.original)
        },
    )?;
    hook("active_cleaned")?;
    clean_stage(
        &archive_stage,
        journal.archive_stage,
        "outgoing",
        if journal.preserve_archive == Some(true) {
            Some(journal.outgoing)
        } else {
            journal.previous_archive
        },
    )?;
    hook("archive_cleaned")?;
    fs::remove_file(journal_path(store))?;
    hook("journal_unlinked")?;
    storage::sync_directory(&store.v2())
}
fn clean_stage(
    stage: &Path,
    expected: Identity,
    child: &str,
    child_identity: Option<Identity>,
) -> io::Result<()> {
    let Some(id) = optional_identity(stage)? else {
        return Ok(());
    };
    let remaining = optional_identity(&stage.join(child))?;
    if id != expected || remaining.is_some_and(|identity| Some(identity) != child_identity) {
        return Err(invalid("staging cleanup identity changed"));
    }
    if fs::read_dir(stage)?.any(|entry| {
        entry
            .map(|entry| entry.file_name() != child)
            .unwrap_or(true)
    }) {
        return Err(invalid("unexpected staging file"));
    }
    if remaining.is_some() {
        fs::remove_dir_all(stage.join(child))?;
    }
    fs::remove_dir(stage)?;
    storage::sync_directory(stage.parent().unwrap())
}
fn copy(from: &Path, to: &Path) -> io::Result<()> {
    fn has_special(path: &Path) -> io::Result<bool> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.is_dir() {
            for child in fs::read_dir(path)? {
                if has_special(&child?.path())? {
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            Ok(!metadata.is_file() && !metadata.file_type().is_symlink())
        }
    }
    // Ditto can wait indefinitely on a FIFO. Let it attempt the copy, then report its timeout.
    let timeout = if has_special(from)? {
        std::time::Duration::from_secs(15)
    } else {
        std::time::Duration::from_secs(30 * 60)
    };
    copy_with_timeout(from, to, timeout)
}
fn copy_with_timeout(from: &Path, to: &Path, timeout: std::time::Duration) -> io::Result<()> {
    if to.exists() {
        return Err(invalid("copy destination exists"));
    }
    let before = fingerprint(from)?;
    let mut child = Command::new("/usr/bin/ditto")
        .args(["--rsrc", "--extattr", "--acl"])
        .arg(from)
        .arg(to)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return Err(io::Error::other("ditto could not copy account data")),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match result {
                    Err(error) => error,
                    _ => io::Error::new(
                        io::ErrorKind::TimedOut,
                        "ditto account copy timed out; original data preserved",
                    ),
                });
            }
        }
    }
    if fingerprint(from)? != before || fingerprint(to)? != before {
        return Err(invalid("account copy content differs"));
    }
    sync_copy(to)
}
fn sync_copy(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            sync_copy(&entry?.path())?;
        }
        storage::sync_directory(path)
    } else if metadata.is_file() {
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?
            .sync_all()
    } else {
        Ok(())
    }
}
fn empty_directory_fingerprint() -> String {
    // fingerprint hashes the root's empty relative path and its directory file type.
    let mut digest = Sha256::new();
    digest.update(0_u64.to_le_bytes());
    digest.update(u32::from(libc::S_IFDIR).to_le_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
/// Content proof, not an owner/mode/ACL policy. Never follows symbolic links or opens devices/FIFOs.
fn fingerprint(root: &Path) -> io::Result<String> {
    fn walk(root: &Path, path: &Path, digest: &mut Sha256) -> io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        let relative = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .as_os_str()
            .as_bytes();
        digest.update((relative.len() as u64).to_le_bytes());
        digest.update(relative);
        digest.update((metadata.mode() & u32::from(libc::S_IFMT)).to_le_bytes());
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(path)?;
            let bytes = target.as_os_str().as_bytes();
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        } else if metadata.is_dir() {
            let mut entries = fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                walk(root, &entry.path(), digest)?;
            }
        } else if metadata.is_file() {
            digest.update(metadata.len().to_le_bytes());
            let mut file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?;
            let opened = file.metadata()?;
            if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
                return Err(invalid("account file changed while reading"));
            }
            let mut buffer = [0; 65536];
            loop {
                let size = file.read(&mut buffer)?;
                if size == 0 {
                    break;
                }
                digest.update(&buffer[..size]);
            }
        } else {
            digest.update(metadata.rdev().to_le_bytes());
        }
        Ok(())
    }
    let mut digest = Sha256::new();
    walk(root, root, &mut digest)?;
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    struct Fixture {
        base: PathBuf,
        store: AccountStore,
    }
    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "legacy-ditto-{}-{}",
                std::process::id(),
                NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
            ));
            storage::create_private_directory(&base).unwrap();
            let root = base.join("switcher");
            let active = base.join("Claude");
            storage::create_private_directory(&root).unwrap();
            storage::create_private_directory(&active).unwrap();
            storage::atomic_write(&active.join("session"), b"A-latest").unwrap();
            let store = AccountStore::new(&root, &active, unsafe { libc::geteuid() }).unwrap();
            Self { base, store }
        }
        fn select(&self, account: AccountId) {
            let lock = LifecycleLock::try_acquire(&self.store.v2()).unwrap();
            select(&self.store, account, &lock, &|| Ok(())).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn mark_session_xattr(path: &Path) -> BTreeMap<Vec<u8>, Vec<u8>> {
        let path_string = path_cstring(path).unwrap();
        let name = CString::new("com.claude-switcher.synthetic").unwrap();
        let value = b"legacy-metadata-proof";
        assert_eq!(
            unsafe {
                libc::setxattr(
                    path_string.as_ptr(),
                    name.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                    libc::XATTR_NOFOLLOW,
                )
            },
            0
        );
        extended_attributes(path).unwrap()
    }

    fn saved_archives(store: &AccountStore) -> Vec<(AccountId, Identity, String)> {
        [AccountId::A, AccountId::B]
            .into_iter()
            .map(|account| {
                let archive = store.archive(account);
                storage::create_private_directory(&archive).unwrap();
                storage::atomic_write(&archive.join("session"), account.name().as_bytes()).unwrap();
                (
                    account,
                    identity(&archive).unwrap(),
                    fingerprint(&archive).unwrap(),
                )
            })
            .collect()
    }

    fn assert_no_stages(store: &AccountStore) {
        for parent in [store.active.parent().unwrap().to_owned(), store.archives()] {
            assert!(!fs::read_dir(parent).unwrap().any(|entry| {
                let name = entry.unwrap().file_name();
                let name = name.to_string_lossy();
                name.starts_with(".claude-switcher-tx-") || name.starts_with(".tx-")
            }));
        }
    }

    fn assert_selected(store: &AccountStore, target: AccountId) {
        let state = store.state().unwrap().unwrap();
        assert_eq!(state.account, target);
        assert_eq!(state.directory, identity(&store.active).unwrap());
        assert_eq!(store.selected_account().unwrap(), target);
        assert_eq!(
            fs::read(store.root.join("current")).unwrap(),
            format!("{}\n", target.name()).as_bytes()
        );
        assert!(!journal_path(store).exists());
        assert_no_stages(store);
    }

    type RecoverySnapshot = Vec<(PathBuf, Option<(Identity, String)>)>;

    fn recovery_snapshot(store: &AccountStore, journal: &LegacyJournal) -> RecoverySnapshot {
        let (active_stage, archive_stage) = stages(store, &journal.transaction);
        [
            store.root.clone(),
            store.active.clone(),
            store.archive(AccountId::A),
            store.archive(AccountId::B),
            active_stage.clone(),
            active_stage.join("incoming"),
            archive_stage.clone(),
            archive_stage.join("outgoing"),
        ]
        .into_iter()
        .map(|path| {
            let snapshot = optional_identity(&path)
                .unwrap()
                .map(|id| (id, fingerprint(&path).unwrap()));
            (path, snapshot)
        })
        .collect()
    }

    fn assert_recovery_snapshot_unchanged(snapshot: &RecoverySnapshot) {
        for (path, before) in snapshot {
            let after = optional_identity(path)
                .unwrap()
                .map(|id| (id, fingerprint(path).unwrap()));
            assert_eq!(&after, before, "recovery changed {}", path.display());
        }
    }

    #[test]
    fn f01_sigkill_child_fixture() {
        let Some(base) = std::env::var_os("SWITCHER_F01_CRASH_BASE") else {
            return;
        };
        let base = PathBuf::from(base);
        assert_eq!(
            base.parent().unwrap(),
            std::env::temp_dir().canonicalize().unwrap()
        );
        assert!(
            base.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("legacy-ditto-")
        );
        storage::validate_no_symlinks(&base).unwrap();
        let target = match std::env::var("SWITCHER_F01_CRASH_TARGET").unwrap().as_str() {
            "A" => AccountId::A,
            "B" => AccountId::B,
            _ => panic!("invalid synthetic account"),
        };
        let phase = std::env::var("SWITCHER_F01_CRASH_PHASE").unwrap();
        let store = AccountStore::new(&base.join("switcher"), &base.join("Claude"), unsafe {
            libc::geteuid()
        })
        .unwrap();
        let lock = LifecycleLock::try_acquire(&store.v2()).unwrap();
        select_with(&store, target, &lock, &|| Ok(()), &mut |point| {
            if point == phase {
                unsafe { libc::raise(libc::SIGKILL) };
                panic!("SIGKILL did not terminate synthetic child");
            }
            Ok(())
        })
        .unwrap();
        panic!("synthetic crash boundary was not reached");
    }

    #[test]
    fn f01_sigkill_preserves_archives_at_every_durable_boundary() {
        use std::os::unix::process::ExitStatusExt;
        for target in [AccountId::A, AccountId::B] {
            for phase in [
                "journal_written",
                "prepared",
                "active_exchanged",
                "swapped",
                "archived",
                "committed",
                "mirrored",
                "active_cleaned",
                "archive_cleaned",
                "journal_unlinked",
            ] {
                let fixture = Fixture::new();
                fs::remove_file(fixture.store.active.join("session")).unwrap();
                let mut saved = Vec::new();
                for account in [AccountId::A, AccountId::B] {
                    let archive = fixture.store.archive(account);
                    storage::create_private_directory(&archive).unwrap();
                    storage::atomic_write(&archive.join("session"), account.name().as_bytes())
                        .unwrap();
                    saved.push((
                        account,
                        identity(&archive).unwrap(),
                        fingerprint(&archive).unwrap(),
                    ));
                }
                let output = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "accounts::legacy::tests::f01_sigkill_child_fixture",
                        "--nocapture",
                    ])
                    .env("SWITCHER_F01_CRASH_BASE", &fixture.base)
                    .env("SWITCHER_F01_CRASH_TARGET", target.name())
                    .env("SWITCHER_F01_CRASH_PHASE", phase)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .unwrap();
                assert_eq!(
                    output.status.signal(),
                    Some(libc::SIGKILL),
                    "{target:?}/{phase}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let store = AccountStore::new(
                    &fixture.store.root,
                    &fixture.store.active,
                    fixture.store.owner,
                )
                .unwrap();
                for _ in 0..2 {
                    let lock = LifecycleLock::try_acquire(&store.v2()).unwrap();
                    recover(&store, &lock, &|| Ok(())).unwrap();
                    for (account, id, hash) in &saved {
                        assert_eq!(identity(&store.archive(*account)).unwrap(), *id);
                        assert_eq!(fingerprint(&store.archive(*account)).unwrap(), *hash);
                    }
                    assert_eq!(
                        fs::read(store.active.join("session")).unwrap(),
                        target.name().as_bytes()
                    );
                    assert_selected(&store, target);
                }
                println!(
                    "F01 target={} boundary={phase} signal=SIGKILL recoveries=2 archives/active/current/journal=ok",
                    target.name()
                );
            }
        }
    }

    #[test]
    fn f01_unowned_empty_prejournal_errors_remove_only_attempt_stages() {
        for target in [AccountId::A, AccountId::B] {
            for phase in [
                "stages_created",
                "source_snapshotted",
                "outgoing_copied",
                "incoming_copied",
                "before_journal",
            ] {
                let fixture = Fixture::new();
                fs::remove_file(fixture.store.active.join("session")).unwrap();
                let saved = saved_archives(&fixture.store);
                let active = (
                    identity(&fixture.store.active).unwrap(),
                    fingerprint(&fixture.store.active).unwrap(),
                );
                let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
                select_with(&fixture.store, target, &lock, &|| Ok(()), &mut |point| {
                    if point == phase {
                        Err(io::Error::other("synthetic prejournal failure"))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
                for (account, id, hash) in saved {
                    assert_eq!(identity(&fixture.store.archive(account)).unwrap(), id);
                    assert_eq!(fingerprint(&fixture.store.archive(account)).unwrap(), hash);
                }
                assert_eq!(identity(&fixture.store.active).unwrap(), active.0);
                assert_eq!(fingerprint(&fixture.store.active).unwrap(), active.1);
                assert!(!journal_path(&fixture.store).exists());
                assert!(!fixture.store.state_path().exists());
                assert!(!fixture.store.root.join("current").exists());
                assert_no_stages(&fixture.store);
            }
        }
    }

    #[test]
    fn f01_committed_cleanup_recovers_after_child_removal() {
        for target in [AccountId::A, AccountId::B] {
            for remove_incoming in [false, true] {
                let fixture = Fixture::new();
                fs::remove_file(fixture.store.active.join("session")).unwrap();
                let saved = saved_archives(&fixture.store);
                let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
                select_with(&fixture.store, target, &lock, &|| Ok(()), &mut |point| {
                    if point == "committed" {
                        Err(io::Error::other("synthetic cleanup pause"))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
                let journal: LegacyJournal =
                    storage::read_json(&journal_path(&fixture.store)).unwrap();
                let (active_stage, archive_stage) = stages(&fixture.store, &journal.transaction);
                let (stage, child) = if remove_incoming {
                    (active_stage, "incoming")
                } else {
                    (archive_stage, "outgoing")
                };
                fs::remove_dir_all(stage.join(child)).unwrap();
                assert!(stage.exists());
                for _ in 0..2 {
                    recover(&fixture.store, &lock, &|| Ok(())).unwrap();
                    for (account, id, hash) in &saved {
                        assert_eq!(identity(&fixture.store.archive(*account)).unwrap(), *id);
                        assert_eq!(
                            fingerprint(&fixture.store.archive(*account)).unwrap(),
                            *hash
                        );
                    }
                    assert_eq!(
                        fs::read(fixture.store.active.join("session")).unwrap(),
                        target.name().as_bytes()
                    );
                    assert_selected(&fixture.store, target);
                }
            }
        }
    }

    #[test]
    fn f01_old_empty_journals_without_at_risk_a_archive_still_recover() {
        for before in [AccountId::A, AccountId::B] {
            let fixture = Fixture::new();
            fs::remove_file(fixture.store.active.join("session")).unwrap();
            storage::atomic_write(
                &fixture.store.root.join("current"),
                format!("{}\n", before.name()).as_bytes(),
            )
            .unwrap();
            let target = if before == AccountId::A {
                AccountId::B
            } else {
                AccountId::A
            };
            if before == AccountId::B {
                let archive = fixture.store.archive(AccountId::B);
                storage::create_private_directory(&archive).unwrap();
                storage::atomic_write(&archive.join("session"), b"B-old").unwrap();
            }
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            select_with(&fixture.store, target, &lock, &|| Ok(()), &mut |point| {
                if point == "prepared" {
                    Err(io::Error::other("synthetic old safe journal"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            let mut old: serde_json::Value =
                storage::read_json(&journal_path(&fixture.store)).unwrap();
            old.as_object_mut().unwrap().remove("preserve_archive");
            old.as_object_mut().unwrap().remove("in_place");
            storage::atomic_json(&journal_path(&fixture.store), &old).unwrap();
            for _ in 0..2 {
                recover(&fixture.store, &lock, &|| Ok(())).unwrap();
                assert_selected(&fixture.store, target);
                assert_eq!(
                    fingerprint(&fixture.store.active).unwrap(),
                    empty_directory_fingerprint()
                );
                assert_eq!(
                    fingerprint(&fixture.store.archive(before)).unwrap(),
                    empty_directory_fingerprint()
                );
            }
        }
    }

    #[test]
    fn f01_old_empty_first_a_in_place_journal_stops_before_recovery_writes() {
        for phase in [
            "journal_written",
            "prepared",
            "archived",
            "committed",
            "mirrored",
            "active_cleaned",
            "archive_cleaned",
        ] {
            let fixture = Fixture::new();
            fs::remove_file(fixture.store.active.join("session")).unwrap();
            let archive = fixture.store.archive(AccountId::A);
            storage::create_private_directory(&archive).unwrap();
            storage::atomic_write(&archive.join("session"), b"A-saved").unwrap();
            let saved = identity(&archive).unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            // Materialize the historical first-A shape. The fixed selector cannot emit it.
            select_with(
                &fixture.store,
                AccountId::A,
                &lock,
                &|| Ok(()),
                &mut |point| {
                    if point == "prepared" {
                        Err(io::Error::other("synthetic historical fixture"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            let mut journal: LegacyJournal =
                storage::read_json(&journal_path(&fixture.store)).unwrap();
            let (active_stage, archive_stage) = stages(&fixture.store, &journal.transaction);
            fs::remove_dir_all(active_stage.join("incoming")).unwrap();
            journal.in_place = true;
            journal.preserve_archive = Some(false);
            journal.incoming = journal.original;
            journal.incoming_hash = journal.original_hash.clone();
            storage::atomic_json(&journal_path(&fixture.store), &journal).unwrap();
            if !["journal_written", "prepared"].contains(&phase) {
                finish(&fixture.store, &journal, &lock, &|| Ok(()), &mut |point| {
                    if point == phase {
                        Err(io::Error::other("synthetic historical boundary"))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            }
            let mut old = serde_json::to_value(&journal).unwrap();
            old.as_object_mut().unwrap().remove("preserve_archive");
            storage::atomic_json(&journal_path(&fixture.store), &old).unwrap();
            let snapshot = recovery_snapshot(&fixture.store, &journal);
            let saved_path = if optional_identity(&archive).unwrap() == Some(saved) {
                archive.clone()
            } else {
                archive_stage.join("outgoing")
            };
            if phase != "archive_cleaned" {
                assert_eq!(identity(&saved_path).unwrap(), saved);
                assert_eq!(fs::read(saved_path.join("session")).unwrap(), b"A-saved");
            } else {
                assert!(
                    !saved_path.exists(),
                    "historical cleanup already removed saved A"
                );
            }
            for _ in 0..2 {
                let error = recover(&fixture.store, &lock, &|| Ok(())).unwrap_err();
                assert!(error.to_string().contains("manual recovery"));
                assert_recovery_snapshot_unchanged(&snapshot);
            }
        }
    }

    #[test]
    fn f01_mismatched_outgoing_journal_stops_before_recovery_mutation() {
        for old_format in [false, true] {
            let fixture = Fixture::new();
            let archive = fixture.store.archive(AccountId::A);
            storage::create_private_directory(&archive).unwrap();
            storage::atomic_write(&archive.join("session"), b"A-saved").unwrap();
            let saved = identity(&archive).unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            select_with(
                &fixture.store,
                AccountId::B,
                &lock,
                &|| Ok(()),
                &mut |point| {
                    if point == "prepared" {
                        Err(io::Error::other("synthetic pause"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            let mut journal: serde_json::Value =
                storage::read_json(&journal_path(&fixture.store)).unwrap();
            let (_, stage) = stages(&fixture.store, journal["transaction"].as_str().unwrap());
            fs::remove_file(stage.join("outgoing/session")).unwrap();
            journal["outgoing_hash"] = serde_json::json!(empty_directory_fingerprint());
            if old_format {
                journal.as_object_mut().unwrap().remove("preserve_archive");
            }
            storage::atomic_json(&journal_path(&fixture.store), &journal).unwrap();
            let before = fs::read(journal_path(&fixture.store)).unwrap();
            let parsed: LegacyJournal = storage::read_json(&journal_path(&fixture.store)).unwrap();
            let snapshot = recovery_snapshot(&fixture.store, &parsed);
            for _ in 0..2 {
                assert!(recover(&fixture.store, &lock, &|| Ok(())).is_err());
                assert_eq!(identity(&archive).unwrap(), saved);
                assert_eq!(fs::read(archive.join("session")).unwrap(), b"A-saved");
                assert_eq!(fs::read(journal_path(&fixture.store)).unwrap(), before);
                assert_eq!(
                    fs::read(fixture.store.active.join("session")).unwrap(),
                    b"A-latest"
                );
                assert!(stage.join("outgoing").exists());
                assert_recovery_snapshot_unchanged(&snapshot);
            }
        }
    }

    #[test]
    fn f01_source_restored_after_empty_copy_is_rejected() {
        for target in [AccountId::A, AccountId::B] {
            let fixture = Fixture::new();
            for account in [AccountId::A, AccountId::B] {
                let archive = fixture.store.archive(account);
                storage::create_private_directory(&archive).unwrap();
                storage::atomic_write(&archive.join("session"), account.name().as_bytes()).unwrap();
            }
            let saved = [AccountId::A, AccountId::B].map(|account| {
                let archive = fixture.store.archive(account);
                (
                    account,
                    identity(&archive).unwrap(),
                    fingerprint(&archive).unwrap(),
                )
            });
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            let result = select_with(&fixture.store, target, &lock, &|| Ok(()), &mut |point| {
                if point == "source_snapshotted" {
                    fs::remove_file(fixture.store.active.join("session"))?;
                } else if point == "outgoing_copied" {
                    storage::atomic_write(&fixture.store.active.join("session"), b"A-latest")?;
                }
                Ok(())
            });
            assert!(
                result.is_err(),
                "empty copy accepted after source restored: {target:?}"
            );
            for (account, id, hash) in saved {
                let archive = fixture.store.archive(account);
                assert_eq!(identity(&archive).unwrap(), id);
                assert_eq!(fingerprint(&archive).unwrap(), hash);
            }
            assert_eq!(
                fs::read(fixture.store.active.join("session")).unwrap(),
                b"A-latest"
            );
            assert!(!journal_path(&fixture.store).exists());
            assert!(!fixture.store.root.join("current").exists());
            assert!(!fixture.store.state_path().exists());
            assert_eq!(fs::read_dir(fixture.store.archives()).unwrap().count(), 2);
            assert_eq!(fs::read_dir(&fixture.base).unwrap().count(), 2);
        }
    }

    #[test]
    fn f01_empty_outgoing_copy_cannot_replace_saved_archive() {
        for target in [AccountId::A, AccountId::B] {
            let fixture = Fixture::new();
            let archive = fixture.store.archive(AccountId::A);
            storage::create_private_directory(&archive).unwrap();
            storage::atomic_write(&archive.join("session"), b"A-saved").unwrap();
            let saved = identity(&archive).unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            let result = select_with(&fixture.store, target, &lock, &|| Ok(()), &mut |point| {
                if point == "outgoing_copied" {
                    for stage in fs::read_dir(fixture.store.archives())? {
                        let outgoing = stage?.path().join("outgoing/session");
                        if outgoing.exists() {
                            fs::remove_file(outgoing)?;
                        }
                    }
                }
                Ok(())
            });
            assert!(result.is_err(), "empty outgoing was accepted: {target:?}");
            assert_eq!(identity(&archive).unwrap(), saved);
            assert_eq!(fs::read(archive.join("session")).unwrap(), b"A-saved");
            assert_eq!(
                fs::read(fixture.store.active.join("session")).unwrap(),
                b"A-latest"
            );
            assert!(!journal_path(&fixture.store).exists());
        }
    }

    #[test]
    fn f01_source_becoming_empty_before_snapshot_preserves_archive() {
        for target in [AccountId::A, AccountId::B] {
            let fixture = Fixture::new();
            let archive = fixture.store.archive(AccountId::A);
            storage::create_private_directory(&archive).unwrap();
            storage::atomic_write(&archive.join("session"), b"A-saved").unwrap();
            let saved = identity(&archive).unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            select_with(&fixture.store, target, &lock, &|| Ok(()), &mut |point| {
                if point == "stages_created" {
                    fs::remove_file(fixture.store.active.join("session"))?;
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(identity(&archive).unwrap(), saved);
            assert_eq!(fs::read(archive.join("session")).unwrap(), b"A-saved");
            assert_eq!(fixture.store.selected_account().unwrap(), target);
        }
    }

    #[test]
    fn f01_unowned_empty_active_recovers_every_durable_boundary() {
        for target in [AccountId::A, AccountId::B] {
            for phase in [
                "journal_written",
                "prepared",
                "active_exchanged",
                "swapped",
                "archived",
                "committed",
                "mirrored",
                "active_cleaned",
                "archive_cleaned",
                "journal_unlinked",
            ] {
                let fixture = Fixture::new();
                fs::remove_file(fixture.store.active.join("session")).unwrap();
                for account in [AccountId::A, AccountId::B] {
                    let archive = fixture.store.archive(account);
                    storage::create_private_directory(&archive).unwrap();
                    storage::atomic_write(&archive.join("session"), account.name().as_bytes())
                        .unwrap();
                }
                let saved_a = identity(&fixture.store.archive(AccountId::A)).unwrap();
                let saved_b = identity(&fixture.store.archive(AccountId::B)).unwrap();
                let hash_a = fingerprint(&fixture.store.archive(AccountId::A)).unwrap();
                let hash_b = fingerprint(&fixture.store.archive(AccountId::B)).unwrap();
                let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
                select_with(&fixture.store, target, &lock, &|| Ok(()), &mut |point| {
                    if point == phase {
                        Err(io::Error::other("synthetic F01 interruption"))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
                recover(&fixture.store, &lock, &|| Ok(())).unwrap();
                recover(&fixture.store, &lock, &|| Ok(())).unwrap();
                for (account, original) in [(AccountId::A, saved_a), (AccountId::B, saved_b)] {
                    assert_eq!(
                        identity(&fixture.store.archive(account)).unwrap(),
                        original,
                        "archive identity changed: target={target:?}, phase={phase}"
                    );
                    assert_eq!(
                        fs::read(fixture.store.archive(account).join("session")).unwrap(),
                        account.name().as_bytes()
                    );
                }
                assert_eq!(
                    fingerprint(&fixture.store.archive(AccountId::A)).unwrap(),
                    hash_a
                );
                assert_eq!(
                    fingerprint(&fixture.store.archive(AccountId::B)).unwrap(),
                    hash_b
                );
                assert_eq!(
                    fs::read(fixture.store.active.join("session")).unwrap(),
                    target.name().as_bytes()
                );
                assert_selected(&fixture.store, target);
            }
        }
    }

    #[test]
    fn f01_preexisting_empty_outgoing_journal_is_preserved_for_review() {
        for phase in [
            "journal_written",
            "prepared",
            "active_exchanged",
            "swapped",
            "archived",
            "committed",
            "mirrored",
            "active_cleaned",
            "archive_cleaned",
        ] {
            let fixture = Fixture::new();
            // Explicit A generates the old empty-outgoing transaction, even with the fix.
            storage::atomic_write(&fixture.store.root.join("current"), b"A\n").unwrap();
            fs::remove_file(fixture.store.active.join("session")).unwrap();
            let archive = fixture.store.archive(AccountId::A);
            storage::create_private_directory(&archive).unwrap();
            storage::atomic_write(&archive.join("session"), b"A-saved").unwrap();
            let saved = identity(&archive).unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            select_with(
                &fixture.store,
                AccountId::B,
                &lock,
                &|| Ok(()),
                &mut |point| {
                    if point == phase {
                        Err(io::Error::other("synthetic old pending transaction"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            let mut old: serde_json::Value =
                storage::read_json(&journal_path(&fixture.store)).unwrap();
            old.as_object_mut().unwrap().remove("preserve_archive");
            storage::atomic_json(&journal_path(&fixture.store), &old).unwrap();
            let journal: LegacyJournal = storage::read_json(&journal_path(&fixture.store)).unwrap();
            let (active_stage, archive_stage) = stages(&fixture.store, &journal.transaction);
            let saved_path = if optional_identity(&archive).unwrap() == Some(saved) {
                archive.clone()
            } else {
                archive_stage.join("outgoing")
            };
            if phase != "archive_cleaned" {
                assert_eq!(identity(&saved_path).unwrap(), saved);
            } else {
                assert!(
                    !saved_path.exists(),
                    "historical cleanup already removed saved A"
                );
            }
            let snapshot = recovery_snapshot(&fixture.store, &journal);
            let before_journal = fs::read(journal_path(&fixture.store)).unwrap();
            let before_active = fingerprint(&fixture.store.active).unwrap();
            let before_state = fs::read(fixture.store.state_path()).ok();
            let before_current = fs::read(fixture.store.root.join("current")).unwrap();
            for _ in 0..2 {
                let error = recover(&fixture.store, &lock, &|| Ok(())).unwrap_err();
                assert!(
                    error.to_string().contains("manual recovery"),
                    "{phase}: {error}"
                );
                if phase != "archive_cleaned" {
                    assert_eq!(identity(&saved_path).unwrap(), saved);
                    assert_eq!(fs::read(saved_path.join("session")).unwrap(), b"A-saved");
                }
                assert_recovery_snapshot_unchanged(&snapshot);
                assert_eq!(
                    fs::read(journal_path(&fixture.store)).unwrap(),
                    before_journal
                );
                assert_eq!(fingerprint(&fixture.store.active).unwrap(), before_active);
                assert_eq!(fs::read(fixture.store.state_path()).ok(), before_state);
                assert_eq!(
                    fs::read(fixture.store.root.join("current")).unwrap(),
                    before_current
                );
                assert_eq!(archive_stage.exists(), phase != "archive_cleaned");
                assert_eq!(
                    active_stage.exists(),
                    !["active_cleaned", "archive_cleaned"].contains(&phase)
                );
            }
        }
    }

    #[test]
    fn f01_recorded_empty_active_keeps_legacy_archiving() {
        for marker_present in [true, false] {
            let fixture = Fixture::new();
            fixture.select(AccountId::A);
            fs::remove_file(fixture.store.active.join("session")).unwrap();
            if !marker_present {
                fs::remove_file(fixture.store.root.join("current")).unwrap();
            }
            fixture.select(AccountId::B);
            assert_eq!(
                fs::read_dir(fixture.store.archive(AccountId::A))
                    .unwrap()
                    .count(),
                0
            );
            assert_eq!(fixture.store.selected_account().unwrap(), AccountId::B);
        }
    }

    #[test]
    fn f01_empty_directory_fingerprint_matches_disk() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.store.active.join("session")).unwrap();
        for mode in [0o700, 0o777] {
            fs::set_permissions(&fixture.store.active, fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(
                fingerprint(&fixture.store.active).unwrap(),
                empty_directory_fingerprint()
            );
        }
        storage::atomic_write(&fixture.store.active.join(".hidden"), b"not empty").unwrap();
        assert_ne!(
            fingerprint(&fixture.store.active).unwrap(),
            empty_directory_fingerprint()
        );
    }

    #[test]
    fn f01_preserved_archive_rejects_changed_outgoing_staging() {
        for replace_directory in [false, true] {
            let fixture = Fixture::new();
            fs::remove_file(fixture.store.active.join("session")).unwrap();
            let archive = fixture.store.archive(AccountId::A);
            storage::create_private_directory(&archive).unwrap();
            storage::atomic_write(&archive.join("session"), b"A-saved").unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            select_with(
                &fixture.store,
                AccountId::B,
                &lock,
                &|| Ok(()),
                &mut |point| {
                    if point == "prepared" {
                        Err(io::Error::other("synthetic pause"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            let journal: LegacyJournal = storage::read_json(&journal_path(&fixture.store)).unwrap();
            let (_, archive_stage) = stages(&fixture.store, &journal.transaction);
            let outgoing = archive_stage.join("outgoing");
            if replace_directory {
                fs::rename(&outgoing, fixture.base.join("preserved-empty-copy")).unwrap();
                storage::create_private_directory(&outgoing).unwrap();
            } else {
                storage::atomic_write(&outgoing.join("injected"), b"changed").unwrap();
            }
            assert!(recover(&fixture.store, &lock, &|| Ok(())).is_err());
            assert_eq!(fs::read(archive.join("session")).unwrap(), b"A-saved");
            assert!(journal_path(&fixture.store).exists());
            assert!(outgoing.exists());
        }
    }

    #[test]
    fn f01_preserve_flag_cannot_hide_nonempty_outgoing() {
        let fixture = Fixture::new();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        select_with(
            &fixture.store,
            AccountId::B,
            &lock,
            &|| Ok(()),
            &mut |point| {
                if point == "prepared" {
                    Err(io::Error::other("synthetic pause"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        let mut journal: serde_json::Value =
            storage::read_json(&journal_path(&fixture.store)).unwrap();
        journal["preserve_archive"] = serde_json::json!(true);
        storage::atomic_json(&journal_path(&fixture.store), &journal).unwrap();
        assert!(recover(&fixture.store, &lock, &|| Ok(())).is_err());
        assert_eq!(
            fs::read(fixture.store.active.join("session")).unwrap(),
            b"A-latest"
        );
        assert!(journal_path(&fixture.store).exists());
    }

    #[test]
    fn first_a_and_missing_same_account_archive_preserve_live_identity() {
        let fixture = Fixture::new();
        symlink("session", fixture.store.active.join("session-link")).unwrap();
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow read,write"])
                .arg(fixture.store.active.join("session"))
                .status()
                .unwrap()
                .success()
        );
        let original = identity(&fixture.store.active).unwrap();
        let content = fingerprint(&fixture.store.active).unwrap();
        for _ in 0..2 {
            fixture.select(AccountId::A);
            assert_eq!(identity(&fixture.store.active).unwrap(), original);
            assert_eq!(fingerprint(&fixture.store.active).unwrap(), content);
            assert_eq!(
                fingerprint(&fixture.store.archive(AccountId::A)).unwrap(),
                content
            );
            assert_eq!(
                fs::read_link(fixture.store.active.join("session-link")).unwrap(),
                PathBuf::from("session")
            );
            let acl = Command::new("/bin/ls")
                .arg("-le")
                .arg(fixture.store.active.join("session"))
                .output()
                .unwrap();
            assert!(String::from_utf8_lossy(&acl.stdout).contains("everyone allow read,write"));
            fs::remove_dir_all(fixture.store.archive(AccountId::A)).unwrap();
        }
    }

    #[test]
    fn in_place_adoption_recovers_every_durable_boundary_without_replacing_live() {
        for phase in [
            "prepared",
            "archived",
            "committed",
            "mirrored",
            "active_cleaned",
            "archive_cleaned",
        ] {
            let fixture = Fixture::new();
            let original = identity(&fixture.store.active).unwrap();
            let content = fingerprint(&fixture.store.active).unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            select_with(
                &fixture.store,
                AccountId::A,
                &lock,
                &|| Ok(()),
                &mut |point| {
                    if point == phase {
                        Err(io::Error::other("synthetic interruption"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            recover(&fixture.store, &lock, &|| Ok(())).unwrap();
            recover(&fixture.store, &lock, &|| Ok(())).unwrap();
            assert_eq!(
                identity(&fixture.store.active).unwrap(),
                original,
                "{phase}"
            );
            assert_eq!(fingerprint(&fixture.store.active).unwrap(), content);
            assert_eq!(
                fingerprint(&fixture.store.archive(AccountId::A)).unwrap(),
                content
            );
            assert!(!journal_path(&fixture.store).exists());
        }
    }

    #[test]
    fn pending_journal_without_in_place_field_retains_exchange_recovery() {
        let fixture = Fixture::new();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        select_with(
            &fixture.store,
            AccountId::B,
            &lock,
            &|| Ok(()),
            &mut |point| {
                if point == "prepared" {
                    Err(io::Error::other("synthetic interruption"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        let mut old: serde_json::Value = storage::read_json(&journal_path(&fixture.store)).unwrap();
        old.as_object_mut().unwrap().remove("in_place");
        old.as_object_mut().unwrap().remove("preserve_archive");
        storage::atomic_json(&journal_path(&fixture.store), &old).unwrap();
        recover(&fixture.store, &lock, &|| Ok(())).unwrap();
        assert_eq!(fixture.store.selected_account().unwrap(), AccountId::B);
        assert_eq!(
            fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
            b"A-latest"
        );
    }

    #[test]
    fn pre_journal_failures_remove_stages_and_preserve_accounts() {
        for point in [
            "stages_created",
            "outgoing_copied",
            "incoming_copied",
            "before_journal",
        ] {
            let fixture = Fixture::new();
            fixture.select(AccountId::A);
            let original = fingerprint(&fixture.store.active).unwrap();
            let archive = fingerprint(&fixture.store.archive(AccountId::A)).unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            for _ in 0..2 {
                select_with(
                    &fixture.store,
                    AccountId::B,
                    &lock,
                    &|| Ok(()),
                    &mut |phase| {
                        if phase == point {
                            Err(io::Error::other("synthetic preparation failure"))
                        } else {
                            Ok(())
                        }
                    },
                )
                .unwrap_err();
                assert!(!journal_path(&fixture.store).exists());
                for parent in [
                    fixture.store.active.parent().unwrap().to_owned(),
                    fixture.store.archives(),
                ] {
                    assert!(
                        !fs::read_dir(parent).unwrap().any(|entry| {
                            let name = entry.unwrap().file_name();
                            let name = name.to_string_lossy();
                            name.starts_with(".claude-switcher-tx-") || name.starts_with(".tx-")
                        }),
                        "orphan stage at {point}"
                    );
                }
                assert_eq!(fingerprint(&fixture.store.active).unwrap(), original);
                assert_eq!(
                    fingerprint(&fixture.store.archive(AccountId::A)).unwrap(),
                    archive
                );
                assert_eq!(fixture.store.selected_account().unwrap(), AccountId::A);
            }
        }
    }

    #[test]
    fn journal_write_error_after_rename_preserves_recoverable_stages() {
        let fixture = Fixture::new();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        select_with(
            &fixture.store,
            AccountId::B,
            &lock,
            &|| Ok(()),
            &mut |phase| {
                if phase == "journal_written" {
                    Err(io::Error::other("synthetic fsync error"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        let journal: LegacyJournal = storage::read_json(&journal_path(&fixture.store)).unwrap();
        let (active, archive) = stages(&fixture.store, &journal.transaction);
        assert!(active.join("incoming").exists());
        assert!(archive.join("outgoing").exists());
        recover(&fixture.store, &lock, &|| Ok(())).unwrap();
        assert_eq!(fixture.store.selected_account().unwrap(), AccountId::B);
        assert_eq!(
            fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
            b"A-latest"
        );
        assert!(!active.exists());
        assert!(!archive.exists());
    }

    #[test]
    fn ditto_roundtrip_preserves_symlinks_hardlink_content_and_writable_modes() {
        let fixture = Fixture::new();
        let outside = fixture.base.join("outside");
        storage::create_private_directory(&outside).unwrap();
        storage::atomic_write(&outside.join("do-not-copy"), b"outside-secret").unwrap();
        symlink(&outside, fixture.store.active.join("outside-link")).unwrap();
        symlink("missing-target", fixture.store.active.join("dangling-link")).unwrap();
        fs::hard_link(
            fixture.store.active.join("session"),
            fixture.store.active.join("hard-link"),
        )
        .unwrap();
        fs::set_permissions(
            fixture.store.active.join("session"),
            fs::Permissions::from_mode(0o666),
        )
        .unwrap();
        fs::set_permissions(&fixture.store.active, fs::Permissions::from_mode(0o777)).unwrap();
        let expected_xattrs = mark_session_xattr(&fixture.store.active.join("session"));
        fixture.select(AccountId::B);
        storage::atomic_write(&fixture.store.active.join("session"), b"B-live").unwrap();
        fixture.select(AccountId::A);
        assert_eq!(
            fs::read_link(fixture.store.active.join("outside-link")).unwrap(),
            outside
        );
        assert_eq!(
            fs::read_link(fixture.store.active.join("dangling-link")).unwrap(),
            PathBuf::from("missing-target")
        );
        assert_eq!(
            fs::read(fixture.store.active.join("hard-link")).unwrap(),
            b"A-latest"
        );
        assert_eq!(
            fs::metadata(fixture.store.active.join("session"))
                .unwrap()
                .mode()
                & 0o777,
            0o666
        );
        assert_eq!(
            extended_attributes(&fixture.store.active.join("session")).unwrap(),
            expected_xattrs
        );
        assert_eq!(
            fs::metadata(&fixture.store.active).unwrap().mode() & 0o777,
            0o777
        );
        assert_eq!(
            fs::read(outside.join("do-not-copy")).unwrap(),
            b"outside-secret"
        );
        assert!(
            fs::symlink_metadata(fixture.store.archive(AccountId::A).join("outside-link"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    #[test]
    fn writable_acl_is_copied_without_becoming_a_business_gate() {
        let fixture = Fixture::new();
        let session = fixture.store.active.join("session");
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow read,write"])
                .arg(&session)
                .status()
                .unwrap()
                .success()
        );
        fixture.select(AccountId::B);
        fixture.select(AccountId::A);
        let output = Command::new("/bin/ls")
            .arg("-le")
            .arg(&session)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("everyone allow read,write"));
        assert_eq!(fs::read(session).unwrap(), b"A-latest");
    }
    #[test]
    fn destination_local_stages_resume_every_persisted_boundary_without_receipts() {
        for point in [
            "prepared",
            "swapped",
            "archived",
            "committed",
            "mirrored",
            "active_cleaned",
            "archive_cleaned",
        ] {
            let fixture = Fixture::new();
            let expected_xattrs = mark_session_xattr(&fixture.store.active.join("session"));
            fs::set_permissions(
                fixture.store.active.join("session"),
                fs::Permissions::from_mode(0o640),
            )
            .unwrap();
            fixture.select(AccountId::A);
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            select_with(
                &fixture.store,
                AccountId::B,
                &lock,
                &|| Ok(()),
                &mut |phase| {
                    if phase == point {
                        Err(io::Error::other("synthetic interruption"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            let journal: LegacyJournal = storage::read_json(&journal_path(&fixture.store)).unwrap();
            let (active_stage, archive_stage) = stages(&fixture.store, &journal.transaction);
            assert_eq!(active_stage.parent(), fixture.store.active.parent());
            assert_eq!(
                archive_stage.parent(),
                Some(fixture.store.archives().as_path())
            );
            recover(&fixture.store, &lock, &|| Ok(())).unwrap();
            recover(&fixture.store, &lock, &|| Ok(())).unwrap();
            assert_eq!(
                fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
                b"A-latest"
            );
            let archived_session = fixture.store.archive(AccountId::A).join("session");
            assert_eq!(
                fs::metadata(&archived_session).unwrap().mode() & 0o777,
                0o640
            );
            assert_eq!(
                extended_attributes(&archived_session).unwrap(),
                expected_xattrs
            );
            assert_eq!(
                fs::read(fixture.store.root.join("current")).unwrap(),
                b"B\n"
            );
            assert_eq!(fs::read_dir(&fixture.store.active).unwrap().count(), 0);
            assert!(!active_stage.exists());
            assert!(!archive_stage.exists());
            assert!(!journal_path(&fixture.store).exists());
        }
    }
    #[test]
    fn unsupported_fifo_times_out_in_native_ditto_without_touching_source() {
        let fixture = Fixture::new();
        let fifo = fixture.store.active.join("pipe");
        let path = path_cstring(&fifo).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let result = copy_with_timeout(
            &fixture.store.active,
            &fixture.base.join("copy"),
            std::time::Duration::from_millis(150),
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert!(std::os::unix::fs::FileTypeExt::is_fifo(
            &fs::symlink_metadata(&fifo).unwrap().file_type()
        ));
        assert_eq!(
            fs::read(fixture.store.active.join("session")).unwrap(),
            b"A-latest"
        );
    }

    #[test]
    fn legacy_process_crash_child() {
        let Some(base) = std::env::var_os("SWITCHER_LEGACY_CRASH_FIXTURE") else {
            return;
        };
        let base = PathBuf::from(base);
        assert_eq!(
            base.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            base.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("legacy-ditto-")
        );
        let store = AccountStore::new(&base.join("switcher"), &base.join("Claude"), unsafe {
            libc::geteuid()
        })
        .unwrap();
        let phase = std::env::var("SWITCHER_LEGACY_CRASH_PHASE").unwrap();
        let lock = LifecycleLock::try_acquire(&store.v2()).unwrap();
        select_with(&store, AccountId::B, &lock, &|| Ok(()), &mut |point| {
            if point == phase {
                storage::atomic_write(&base.join("child-ready"), b"ready")?;
                loop {
                    std::thread::park_timeout(std::time::Duration::from_secs(1));
                }
            }
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn f06_fresh_recovery_child_fixture() {
        let Some(base) = std::env::var_os("SWITCHER_F06_RECOVERY_BASE") else {
            return;
        };
        let base = PathBuf::from(base);
        assert_eq!(
            base.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            base.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("legacy-ditto-")
        );
        storage::validate_no_symlinks(&base).unwrap();
        let metadata = fs::metadata(&base).unwrap();
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.permissions().mode() & 0o077, 0);
        let generation: u64 = std::env::var("SWITCHER_F06_GENERATION")
            .unwrap()
            .parse()
            .unwrap();
        let store = AccountStore::new(&base.join("switcher"), &base.join("Claude"), unsafe {
            libc::geteuid()
        })
        .unwrap();
        let mut first_state = None;
        for _ in 0..2 {
            let lock = LifecycleLock::try_acquire(&store.v2()).unwrap();
            recover(&store, &lock, &|| Ok(())).unwrap();
            assert_selected(&store, AccountId::B);
            let state = store.state().unwrap().unwrap();
            assert_eq!(state.generation, generation);
            if let Some(first) = &first_state {
                assert_eq!(&state, first);
            } else {
                first_state = Some(state);
            }
            assert!(!store.journal_path().exists());
            assert_eq!(fs::read_dir(store.transactions()).unwrap().count(), 0);
            assert_eq!(
                fs::read(store.active.join("session")).unwrap(),
                b"B-private"
            );
            assert_eq!(
                fs::read(store.archive(AccountId::A).join("session")).unwrap(),
                b"A-latest"
            );
            let expected_xattrs =
                extended_attributes(&store.archive(AccountId::B).join("session")).unwrap();
            assert!(!expected_xattrs.is_empty());
            for directory in [
                &store.active,
                &store.archive(AccountId::A),
                &store.archive(AccountId::B),
            ] {
                assert_eq!(
                    fs::metadata(directory).unwrap().permissions().mode() & 0o777,
                    0o750
                );
                let session = directory.join("session");
                assert_eq!(
                    fs::metadata(&session).unwrap().permissions().mode() & 0o777,
                    0o640
                );
                assert_eq!(extended_attributes(&session).unwrap(), expected_xattrs);
            }
        }
        println!(
            "F06 fresh recovery pid={} recoveries=2 metadata/stages/journal=ok",
            std::process::id()
        );
    }

    fn assert_f06_fresh_process_recovery(phases: &[&str]) {
        use std::os::unix::process::ExitStatusExt;
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        for &phase in phases {
            let fixture = Fixture::new();
            fixture.select(AccountId::A);
            let b_archive = fixture.store.archive(AccountId::B);
            storage::create_private_directory(&b_archive).unwrap();
            storage::atomic_write(&b_archive.join("session"), b"B-private").unwrap();
            for directory in [&fixture.store.active, &b_archive] {
                fs::set_permissions(directory, fs::Permissions::from_mode(0o750)).unwrap();
                fs::set_permissions(directory.join("session"), fs::Permissions::from_mode(0o640))
                    .unwrap();
                mark_session_xattr(&directory.join("session"));
            }
            let a_hash = fingerprint(&fixture.store.active).unwrap();
            let b_hash = fingerprint(&b_archive).unwrap();
            let b_identity = identity(&b_archive).unwrap();
            let generation = fixture.store.state().unwrap().unwrap().generation + 1;
            let mut child = ChildGuard(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "accounts::legacy::tests::legacy_process_crash_child",
                        "--nocapture",
                    ])
                    .env("SWITCHER_LEGACY_CRASH_FIXTURE", &fixture.base)
                    .env("SWITCHER_LEGACY_CRASH_PHASE", phase)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            while !fixture.base.join("child-ready").exists() {
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "child exited before {phase}"
                );
                assert!(
                    std::time::Instant::now() < deadline,
                    "child timed out before {phase}"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert_eq!(
                LifecycleLock::try_acquire(&fixture.store.v2())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            if phase == "journal_unlinked" {
                assert!(!journal_path(&fixture.store).exists());
            } else {
                let journal: LegacyJournal =
                    storage::read_json(&journal_path(&fixture.store)).unwrap();
                assert_eq!(journal.target, AccountId::B);
            }
            child.0.kill().unwrap();
            assert_eq!(child.0.wait().unwrap().signal(), Some(libc::SIGKILL));
            let mut recovery = ChildGuard(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "accounts::legacy::tests::f06_fresh_recovery_child_fixture",
                        "--nocapture",
                    ])
                    .env("SWITCHER_F06_RECOVERY_BASE", &fixture.base)
                    .env("SWITCHER_F06_GENERATION", generation.to_string())
                    .stdin(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let status = loop {
                if let Some(status) = recovery.0.try_wait().unwrap() {
                    break status;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "recovery timed out after {phase}"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert!(
                status.success(),
                "fresh recovery failed after {phase}: {status}"
            );
            let store = AccountStore::new(
                &fixture.store.root,
                &fixture.store.active,
                fixture.store.owner,
            )
            .unwrap();
            assert_selected(&store, AccountId::B);
            assert_eq!(store.state().unwrap().unwrap().generation, generation);
            assert_eq!(fingerprint(&store.active).unwrap(), b_hash);
            assert_eq!(fingerprint(&store.archive(AccountId::A)).unwrap(), a_hash);
            assert_eq!(fingerprint(&b_archive).unwrap(), b_hash);
            assert_eq!(identity(&b_archive).unwrap(), b_identity);
            println!(
                "F06 boundary={phase} signal=SIGKILL fresh_recovery_exit=0 recoveries=2 archives/metadata/stages/journal=ok"
            );
        }
    }

    #[test]
    fn f06_missing_switch_boundaries_recover_in_fresh_process() {
        assert_f06_fresh_process_recovery(&[
            "journal_written",
            "active_exchanged",
            "journal_unlinked",
        ]);
    }

    #[test]
    fn f06_other_switch_boundaries_recover_in_fresh_process() {
        assert_f06_fresh_process_recovery(&[
            "prepared",
            "swapped",
            "archived",
            "committed",
            "mirrored",
            "active_cleaned",
            "archive_cleaned",
        ]);
    }

    #[test]
    fn sigkill_recovers_destination_local_transactions_at_every_boundary() {
        for phase in [
            "prepared",
            "swapped",
            "archived",
            "committed",
            "mirrored",
            "active_cleaned",
            "archive_cleaned",
        ] {
            let fixture = Fixture::new();
            fixture.select(AccountId::A);
            struct Child(std::process::Child);
            impl Drop for Child {
                fn drop(&mut self) {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                }
            }
            let mut child = Child(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "accounts::legacy::tests::legacy_process_crash_child",
                        "--nocapture",
                    ])
                    .env("SWITCHER_LEGACY_CRASH_FIXTURE", &fixture.base)
                    .env("SWITCHER_LEGACY_CRASH_PHASE", phase)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !fixture.base.join("child-ready").exists() && std::time::Instant::now() < deadline
            {
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "child exited before {phase}"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert!(
                fixture.base.join("child-ready").exists(),
                "child did not reach {phase}"
            );
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
            recover(&fixture.store, &lock, &|| Ok(())).unwrap();
            recover(&fixture.store, &lock, &|| Ok(())).unwrap();
            assert_eq!(
                fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
                b"A-latest"
            );
            assert_eq!(
                fs::read(fixture.store.root.join("current")).unwrap(),
                b"B\n"
            );
            assert!(!journal_path(&fixture.store).exists());
            println!("legacy phase={phase} SIGKILL recovery=ok");
        }
    }

    #[test]
    fn cleanup_resumes_when_child_removed_before_stage_directory() {
        let fixture = Fixture::new();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        select_with(
            &fixture.store,
            AccountId::B,
            &lock,
            &|| Ok(()),
            &mut |phase| {
                if phase == "committed" {
                    Err(io::Error::other("synthetic interruption"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        let journal: LegacyJournal = storage::read_json(&journal_path(&fixture.store)).unwrap();
        let (stage, _) = stages(&fixture.store, &journal.transaction);
        fs::remove_dir_all(stage.join("incoming")).unwrap();
        recover(&fixture.store, &lock, &|| Ok(())).unwrap();
        assert!(!journal_path(&fixture.store).exists());
        assert!(!stage.exists());
        assert_eq!(
            fs::read(fixture.store.archive(AccountId::A).join("session")).unwrap(),
            b"A-latest"
        );
    }

    #[test]
    fn changed_stage_is_preserved_instead_of_overwriting_account_data() {
        let fixture = Fixture::new();
        let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
        select_with(
            &fixture.store,
            AccountId::B,
            &lock,
            &|| Ok(()),
            &mut |phase| {
                if phase == "prepared" {
                    Err(io::Error::other("synthetic interruption"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        let journal: LegacyJournal = storage::read_json(&journal_path(&fixture.store)).unwrap();
        let (stage, _) = stages(&fixture.store, &journal.transaction);
        storage::atomic_write(&stage.join("incoming/injected"), b"changed").unwrap();
        assert!(recover(&fixture.store, &lock, &|| Ok(())).is_err());
        assert_eq!(
            fs::read(fixture.store.active.join("session")).unwrap(),
            b"A-latest"
        );
        assert!(journal_path(&fixture.store).exists());
    }
}
