//! Explicit native acceptance across two sequential OS processes; no default Keychain writes.
use super::*;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WORKER: &str =
    "publisher::security::native_keychain_tests::process_tests::native_keychain_process_worker";

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    writer_pid: u32,
    original: String,
    restored: String,
    old_state_sha256: String,
}

fn root() -> PathBuf {
    assert!(
        std::env::var("SWITCHER_NATIVE_KEYCHAIN_OPT_IN").as_deref()
            == Ok("synthetic-owned-items-only")
    );
    let root = PathBuf::from(std::env::var_os("SWITCHER_NATIVE_KEYCHAIN_ROOT").unwrap());
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .join("output")
        .canonicalize()
        .unwrap();
    assert!(
        root.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(output)
    );
    root
}

fn private_metadata(path: &Path, directory: bool) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    assert!(if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    });
    assert!(metadata.uid() == unsafe { libc::geteuid() });
    assert!(metadata.mode() & 0o077 == 0);
}

fn open_vault(root: &Path) -> KeychainVault {
    private_metadata(root, true);
    let path = root.join("isolated-a.keychain");
    private_metadata(&path, false);
    let mut keychain = SecKeychain::open(path).unwrap();
    keychain
        .unlock(Some("synthetic-private-keychain-password"))
        .unwrap();
    KeychainVault::isolated(keychain)
}

fn write_json(path: &Path, value: &impl Serialize) {
    switcher_core::storage::atomic_write(path, &serde_json::to_vec(value).unwrap()).unwrap();
}

fn digest(path: &Path) -> String {
    switcher_core::signed::digest(&fs::read(path).unwrap())
}

fn generation(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn writer(root: &Path) {
    let mut vault = OwnedVault::new(root, open_vault(root));
    let publisher_root = root.join("publisher");
    let mut publisher =
        Publisher::open(&publisher_root, "native-synthetic-key", None, 2000).unwrap();
    let original = publisher.generation.clone();
    vault.create(&original, &secrets(0x41)).unwrap();
    publisher
        .credentials_save(&mut vault, &secrets(0x41))
        .unwrap();
    let password = b"native-synthetic-backup-password";
    let backup = zeroize::Zeroizing::new(publisher.export_backup(password, &vault).unwrap());
    publisher.pause(None, 2001).unwrap();
    let old_state_sha256 = digest(&publisher_root.join(&original).join("state.json"));
    publisher
        .restore_backup(&backup, password, &mut vault)
        .unwrap();
    assert!(publisher.generation != original && publisher.state.recovery_required);
    let checkpoint = Checkpoint {
        writer_pid: std::process::id(),
        original,
        restored: publisher.generation.clone(),
        old_state_sha256,
    };
    drop(publisher);
    write_json(&root.join("checkpoint.json"), &checkpoint);
}

fn reader(root: &Path) {
    let vault = open_vault(root);
    private_metadata(&root.join("checkpoint.json"), false);
    let checkpoint: Checkpoint =
        serde_json::from_slice(&fs::read(root.join("checkpoint.json")).unwrap()).unwrap();
    assert!(generation(&checkpoint.original) && generation(&checkpoint.restored));
    assert!(checkpoint.writer_pid != std::process::id());
    let publisher_root = root.join("publisher");
    let publisher = Publisher::open(&publisher_root, "native-synthetic-key", None, 2002).unwrap();
    assert!(publisher.generation == checkpoint.restored);
    assert!(publisher.state.recovery_required);
    same_secrets(&vault.load(&checkpoint.original).unwrap(), &secrets(0x41));
    same_secrets(&vault.load(&checkpoint.restored).unwrap(), &secrets(0x41));
    assert!(
        digest(&publisher_root.join(&checkpoint.original).join("state.json"))
            == checkpoint.old_state_sha256
    );
    assert!(!publisher_root.join("public").exists());
    write_json(
        &root.join("reader.json"),
        &serde_json::json!({
            "writer_pid": checkpoint.writer_pid, "reader_pid": std::process::id(),
            "restored_generation_read": true, "recovery_required": true,
            "old_and_restored_keys_match": true, "old_state_unchanged": true,
            "no_automatic_publication": true
        }),
    );
}

#[test]
#[ignore = "worker invoked only by explicit native process acceptance"]
fn native_keychain_process_worker() {
    let root = root();
    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    match std::env::var("SWITCHER_NATIVE_KEYCHAIN_PHASE").as_deref() {
        Ok("writer") => writer(&root),
        Ok("reader") => reader(&root),
        _ => panic!("explicit native worker phase required"),
    }
}

fn run_worker(root: &Path, phase: &str) -> u32 {
    let log = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(format!("{phase}.log")))
        .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", WORKER, "--ignored", "--test-threads=1"])
        .env("SWITCHER_NATIVE_KEYCHAIN_PHASE", phase)
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .unwrap();
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            write_json(
                &root.join(format!("{phase}-exit.json")),
                &serde_json::json!({
                    "pid": pid, "exit": status.code(), "success": status.success()
                }),
            );
            assert!(
                status.success(),
                "native child failed; inspect its synthetic log"
            );
            return pid;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native child timed out and was reaped");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "explicit native Keychain process restart acceptance; synthetic owned items only"]
fn native_keychain_restore_survives_writer_exit() {
    let root = root();
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    let before = preferences();
    let mut private = PrivateKeychain::create(&root, "isolated-a.keychain");
    // The SDK can create a 0644 file; tighten this owned fixture before child access.
    fs::set_permissions(
        root.join("isolated-a.keychain"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    write_json(
        &root.join("owned-keychain-items.json"),
        &BTreeSet::<String>::new(),
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let writer_pid = run_worker(&root, "writer"); // Reaped before the reader can start.
        let reader_pid = run_worker(&root, "reader");
        assert!(writer_pid != reader_pid && writer_pid != std::process::id());
        assert!(reader_pid != std::process::id());
    }));
    let vault = OwnedVault::new(&root, private.vault());
    // Only exact generation IDs from this newly created private library are eligible.
    let cleaned = match fs::read(root.join("owned-keychain-items.json")) {
        Ok(bytes) => match serde_json::from_slice::<BTreeSet<String>>(&bytes) {
            Ok(items) if items.iter().all(|id| generation(id)) => {
                *vault.items.lock().unwrap() = items;
                vault.cleanup()
            }
            _ => false,
        },
        Err(_) => false,
    };
    let removed = private.remove();
    let unchanged = before == preferences();
    write_json(
        &root.join("result.json"),
        &serde_json::json!({
            "exercise_passed": result.is_ok(), "owned_items_removed": cleaned,
            "private_keychain_removed": removed, "preferences_unchanged": unchanged,
            "scope": "writer exits and is reaped before new reader process; direct Publisher and explicit private Keychain"
        }),
    );
    assert!(
        cleaned && removed && unchanged,
        "native cleanup or preference invariance failed"
    );
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
