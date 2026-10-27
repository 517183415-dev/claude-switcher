//! Real Darwin filesystem assertions, independent of the account fingerprint.
mod volume;
use super::{AccountId, Fixture};
use crate::storage::LifecycleLock;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs, io,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
};

const ATTRIBUTE: &str = "com.claude-switcher.test.metadata";
const NESTED: &str = "嵌套目录";
const LEAF: &str = "嵌套目录/会话.bin";

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Payload {
    mode: u32,
    owner: u32,
    group: u32,
    modified: (i64, i64),
    content: Option<Vec<u8>>,
    attribute: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Entry {
    device: u64,
    inode: u64,
    payload: Payload,
}

type Tree = BTreeMap<String, Entry>;

fn native_attribute(path: &Path, value: Option<&[u8]>) -> Vec<u8> {
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    let name = CString::new(ATTRIBUTE).unwrap();
    if let Some(value) = value {
        let result = unsafe {
            libc::setxattr(
                path.as_ptr(),
                name.as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        assert_eq!(result, 0, "{}", io::Error::last_os_error());
    }
    // Read directly with Darwin, not the production listxattr/fingerprint helper.
    let mut bytes = [0u8; 128];
    let count = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    assert!(count >= 0, "{}", io::Error::last_os_error());
    bytes[..count as usize].to_vec()
}

fn seed(root: &Path, account: &str) {
    fs::create_dir(root.join(NESTED)).unwrap();
    fs::write(root.join(LEAF), format!("{account}-original")).unwrap();
    for (relative, mode) in [
        ("", 0o750),
        (NESTED, 0o710),
        (LEAF, 0o640),
        ("session", 0o600),
    ] {
        let path = root.join(relative);
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        let mut value = format!("{account}/{relative}").into_bytes();
        value.extend_from_slice(&[0, 255]);
        assert_eq!(native_attribute(&path, Some(&value)), value);
    }
}

fn snapshot(root: &Path) -> Tree {
    fn walk(root: &Path, path: &Path, tree: &mut Tree) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let entry = Entry {
            device: metadata.dev(),
            inode: metadata.ino(),
            payload: Payload {
                mode: metadata.mode() & 0o7777,
                owner: metadata.uid(),
                group: metadata.gid(),
                modified: (metadata.mtime(), metadata.mtime_nsec()),
                content: metadata.is_file().then(|| fs::read(path).unwrap()),
                attribute: native_attribute(path, None),
            },
        };
        assert!(tree.insert(relative, entry).is_none());
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                walk(root, &entry.unwrap().path(), tree);
            }
        }
    }
    let mut tree = Tree::new();
    walk(root, root, &mut tree);
    tree
}

fn assert_payloads(source: &Tree, destination: &Tree) {
    assert_eq!(
        source.keys().collect::<Vec<_>>(),
        destination.keys().collect::<Vec<_>>()
    );
    for (path, expected) in source {
        assert_eq!(
            expected.payload, destination[path].payload,
            "{path}: copy metadata or bytes changed"
        );
    }
}

fn assert_copy(source: &Tree, destination: &Tree) {
    assert_payloads(source, destination);
    for (path, expected) in source {
        let actual = &destination[path];
        assert_eq!(expected.device, actual.device, "{path}: volume changed");
        assert_ne!(expected.inode, actual.inode, "{path}: source inode reused");
    }
}

fn filesystem(path: &Path) -> String {
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    assert_eq!(unsafe { libc::statfs(path.as_ptr(), info.as_mut_ptr()) }, 0);
    let info = unsafe { info.assume_init() };
    unsafe { CStr::from_ptr(info.f_fstypename.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

// Match switch_to's production transaction; only process observation is synthetic.
fn select(fixture: &Fixture, account: AccountId) -> io::Result<()> {
    let lock = LifecycleLock::try_acquire(&fixture.store.v2())?;
    fixture
        .store
        .select_legacy_checked(account, &lock, &|| Ok(()))
}

fn assert_clean(fixture: &Fixture) {
    assert!(!fixture.store.journal_path().exists());
    assert!(
        !fixture
            .store
            .v2()
            .join("legacy-account-journal.json")
            .exists()
    );
    for parent in [
        fixture.store.active.parent().unwrap().to_path_buf(),
        fixture.store.archives(),
    ] {
        for entry in fs::read_dir(parent).unwrap() {
            let name = entry.unwrap().file_name();
            let name = name.to_string_lossy();
            assert!(!name.starts_with(".claude-switcher-tx-") && !name.starts_with(".tx-"));
        }
    }
}

#[test]
fn f05_native_round_trip_preserves_both_trees_and_latest_nested_metadata() {
    let fixture = Fixture::new();
    seed(&fixture.store.active, "A");
    select(&fixture, AccountId::A).unwrap();
    fixture.add_b();
    seed(&fixture.store.archive(AccountId::B), "B");
    fs::write(fixture.store.active.join(LEAF), b"A-latest\0\xff").unwrap();
    let a_latest = snapshot(&fixture.store.active);
    let b_original = snapshot(&fixture.store.archive(AccountId::B));
    assert_eq!(a_latest[""].device, b_original[""].device);

    select(&fixture, AccountId::B).unwrap();
    assert_copy(&a_latest, &snapshot(&fixture.store.archive(AccountId::A)));
    assert_eq!(snapshot(&fixture.store.archive(AccountId::B)), b_original);
    let a_archived = snapshot(&fixture.store.archive(AccountId::A));
    let b_active = snapshot(&fixture.store.active);
    assert_copy(&b_original, &b_active);
    assert_eq!(fixture.store.status().unwrap().current, Some(AccountId::B));

    let leaf = fixture.store.active.join(LEAF);
    fs::write(&leaf, b"B-latest\0\xfe").unwrap();
    fs::set_permissions(&leaf, fs::Permissions::from_mode(0o600)).unwrap();
    native_attribute(&leaf, Some(b"B-latest-attribute\0\xfe"));
    let b_latest = snapshot(&fixture.store.active);
    select(&fixture, AccountId::A).unwrap();
    let a_restored = snapshot(&fixture.store.active);
    assert_copy(&a_archived, &a_restored);
    assert_eq!(snapshot(&fixture.store.archive(AccountId::A)), a_archived);
    assert_copy(&b_latest, &snapshot(&fixture.store.archive(AccountId::B)));
    assert_eq!(fixture.store.status().unwrap().current, Some(AccountId::A));
    assert!(!fixture.store.status().unwrap().recovery_pending);
    assert_clean(&fixture);
    assert_eq!(
        fs::read(fixture.store.root.join("current")).unwrap(),
        b"A\n"
    );
    println!(
        "F05_NATIVE_METADATA {}",
        serde_json::json!({
            "filesystem": filesystem(&fixture.base), "a_latest": a_latest,
            "b_original": b_original, "b_latest": b_latest, "a_restored": a_restored
        })
    );
}

#[test]
fn f05_native_running_process_preserves_active_archive_and_committed_state() {
    let fixture = Fixture::new();
    seed(&fixture.store.active, "A");
    select(&fixture, AccountId::A).unwrap();
    let active = snapshot(&fixture.store.active);
    let archive = snapshot(&fixture.store.archive(AccountId::A));
    let state = fs::read(fixture.store.state_path()).unwrap();
    let current = fs::read(fixture.store.root.join("current")).unwrap();
    let lock = LifecycleLock::try_acquire(&fixture.store.v2()).unwrap();
    let error = fixture
        .store
        .select_legacy_checked(AccountId::B, &lock, &|| {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "synthetic Claude still running",
            ))
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(snapshot(&fixture.store.active), active);
    assert_eq!(snapshot(&fixture.store.archive(AccountId::A)), archive);
    assert_eq!(fs::read(fixture.store.state_path()).unwrap(), state);
    assert_eq!(
        fs::read(fixture.store.root.join("current")).unwrap(),
        current
    );
    assert!(!fixture.store.archive(AccountId::B).exists());
    assert_clean(&fixture);
    assert_eq!(
        fs::read_dir(fixture.store.transactions()).unwrap().count(),
        0
    );
    assert!(!fixture.store.status().unwrap().recovery_pending);
    println!(
        "F05_NATIVE_RUNNING_PROCESS {}",
        serde_json::json!({
            "filesystem": filesystem(&fixture.base), "active": active, "archive": archive,
            "state_unchanged": true, "current_unchanged": true, "transaction_count": 0
        })
    );
}
