//! Explicit, ignored tests for a disposable volume supplied by the local runner.
use super::*;
use crate::{accounts::AccountStore, storage};
use std::{io::Write, os::unix::fs::DirBuilderExt, path::PathBuf, sync::atomic::Ordering};

const MIB: u64 = 1024 * 1024;
const PAYLOAD_BYTES: u64 = 32 * MIB;

struct VolumeCase(PathBuf);
impl Drop for VolumeCase {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(active_external: bool) -> (Fixture, VolumeCase) {
    let mut fixture = Fixture::new();
    let mount = PathBuf::from(
        std::env::var_os("SWITCHER_ACCOUNT_TEST_VOLUME")
            .expect("run through tests/native_accounts/run.py with a disposable volume"),
    );
    storage::validate_no_symlinks(&mount).unwrap();
    assert_ne!(
        fs::metadata(&mount).unwrap().dev(),
        fs::metadata(&fixture.base).unwrap().dev(),
        "a real distinct volume is required"
    );
    assert_eq!(filesystem(&mount), "apfs");
    let path = mount.join(format!(
        "f05-case-{}-{}",
        std::process::id(),
        crate::accounts::NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
    ));
    // Arm cleanup only after exclusive creation of this private child.
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    let case = VolumeCase(path);
    let (root, active) = if active_external {
        let active = case.0.join("Claude-synthetic");
        storage::create_private_directory(&active).unwrap();
        fs::write(active.join("session"), b"A-original").unwrap();
        (fixture.store.root.clone(), active)
    } else {
        let root = case.0.join("switcher");
        storage::create_private_directory(&root).unwrap();
        (root, fixture.store.active.clone())
    };
    fixture.store = AccountStore::new(&root, &active, unsafe { libc::geteuid() }).unwrap();
    (fixture, case)
}

fn assert_cross_copy(source: &Tree, destination: &Tree) {
    assert_payloads(source, destination);
    for (path, expected) in source {
        assert_ne!(
            expected.device, destination[path].device,
            "{path}: copy stayed on one volume"
        );
    }
}

fn prepare(fixture: &Fixture) {
    seed(&fixture.store.active, "A");
    select(fixture, AccountId::A).unwrap();
    fixture.add_b();
    seed(&fixture.store.archive(AccountId::B), "B");
}

#[test]
#[ignore = "requires the disposable APFS volume created by tests/native_accounts/run.py"]
fn f05_native_cross_volume_round_trip_preserves_latest_trees() {
    for active_external in [true, false] {
        let (fixture, _case) = fixture(active_external);
        prepare(&fixture);
        fs::write(fixture.store.active.join(LEAF), b"A-cross-volume-latest").unwrap();
        let a_latest = snapshot(&fixture.store.active);
        let b_original = snapshot(&fixture.store.archive(AccountId::B));
        select(&fixture, AccountId::B).unwrap();
        assert_cross_copy(&a_latest, &snapshot(&fixture.store.archive(AccountId::A)));
        assert_eq!(snapshot(&fixture.store.archive(AccountId::B)), b_original);
        assert_cross_copy(&b_original, &snapshot(&fixture.store.active));
        fs::write(fixture.store.active.join(LEAF), b"B-cross-volume-latest").unwrap();
        native_attribute(
            &fixture.store.active.join(LEAF),
            Some(b"B-cross-volume-xattr\0\xff"),
        );
        let b_latest = snapshot(&fixture.store.active);
        let a_archive = snapshot(&fixture.store.archive(AccountId::A));
        select(&fixture, AccountId::A).unwrap();
        assert_cross_copy(&a_archive, &snapshot(&fixture.store.active));
        assert_cross_copy(&b_latest, &snapshot(&fixture.store.archive(AccountId::B)));
        assert_eq!(snapshot(&fixture.store.archive(AccountId::A)), a_archive);
        assert_eq!(fixture.store.status().unwrap().current, Some(AccountId::A));
        assert_eq!(
            fs::read(fixture.store.root.join("current")).unwrap(),
            b"A\n"
        );
        assert_clean(&fixture);
        println!(
            "F05_CROSS_VOLUME active_external={active_external} active_device={} archive_device={} payloads/modes/mtime/xattrs/cleanup=ok",
            a_latest[""].device, b_original[""].device
        );
    }
}

fn available(path: &Path) -> u64 {
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    assert_eq!(
        unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) },
        0
    );
    let stat = unsafe { stat.assume_init() };
    u64::from(stat.f_bavail).saturating_mul(stat.f_frsize)
}

fn fill_disposable_volume(case: &VolumeCase) -> u64 {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(case.0.join("synthetic-filler"))
        .unwrap();
    let error = loop {
        assert!(
            file.metadata().unwrap().len() < 128 * MIB,
            "refuse filling more than the disposable image capacity"
        );
        if let Err(error) = file
            .write_all(&[0x5a; 65536])
            .and_then(|()| file.sync_all())
        {
            break error;
        }
    };
    assert_eq!(error.raw_os_error(), Some(libc::ENOSPC));
    // APFS available-block accounting may include space reserved for metadata.
    // Observe actual ENOSPC, then release enough for staging directories, not the 32 MiB copy.
    let full_length = file.metadata().unwrap().len();
    assert!(full_length > 4 * MIB);
    file.set_len(full_length - 4 * MIB).unwrap();
    file.sync_all().unwrap();
    println!(
        "F05_VOLUME_PRESSURE errno=ENOSPC filler_bytes={full_length} released_bytes={}",
        4 * MIB
    );
    available(&case.0)
}

#[test]
#[ignore = "fills only the disposable APFS volume created by tests/native_accounts/run.py"]
fn f05_native_full_destination_preserves_both_accounts_and_state() {
    for active_external in [false, true] {
        let (fixture, case) = fixture(active_external);
        prepare(&fixture);
        // Root external: outgoing A copy fails. Active external: incoming B copy fails.
        let source = if active_external {
            fixture.store.archive(AccountId::B)
        } else {
            fixture.store.active.clone()
        };
        let large = source.join("synthetic-payload");
        let mut file = fs::File::create(&large).unwrap();
        // Deterministic pseudorandom bytes prevent compression from hiding capacity failures.
        let mut state = 0x2c91_b73d_864a_5ef1u64;
        let mut block = [0u8; 65536];
        for _ in 0..PAYLOAD_BYTES / block.len() as u64 {
            for word in block.chunks_exact_mut(8) {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                word.copy_from_slice(&state.to_le_bytes());
            }
            file.write_all(&block).unwrap();
        }
        file.sync_all().unwrap();
        native_attribute(&large, Some(b"synthetic-volume-pressure"));
        let active = snapshot(&fixture.store.active);
        let a_archive = snapshot(&fixture.store.archive(AccountId::A));
        let b_archive = snapshot(&fixture.store.archive(AccountId::B));
        let state = fs::read(fixture.store.state_path()).unwrap();
        let current = fs::read(fixture.store.root.join("current")).unwrap();
        let remaining = fill_disposable_volume(&case);
        assert_eq!(fs::metadata(&large).unwrap().len(), PAYLOAD_BYTES);
        let error = select(&fixture, AccountId::B).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "ditto could not copy account data");
        assert_eq!(snapshot(&fixture.store.active), active);
        assert_eq!(snapshot(&fixture.store.archive(AccountId::A)), a_archive);
        assert_eq!(snapshot(&fixture.store.archive(AccountId::B)), b_archive);
        assert_eq!(fs::read(fixture.store.state_path()).unwrap(), state);
        assert_eq!(
            fs::read(fixture.store.root.join("current")).unwrap(),
            current
        );
        assert!(!fixture.store.status().unwrap().recovery_pending);
        assert_clean(&fixture);
        println!(
            "F05_FULL_DESTINATION active_external={active_external} remaining={remaining} payload={} error={error} accounts/state/current/cleanup=unchanged",
            PAYLOAD_BYTES
        );
    }
}
