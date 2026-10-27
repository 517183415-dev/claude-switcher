//! Exercise the real account coordinator against the desktop update runtime.
use super::*;
use std::{
    fs,
    os::unix::{
        fs::{DirBuilderExt, MetadataExt},
        process::ExitStatusExt,
    },
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const UPDATE_CHILD: &str = "update_runtime::contention_tests::child_fixture";
const ACCOUNT_CHILD: &str = "migration::update_contention_tests::account_child";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-entry-contention-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        storage::create_private_directory(&root.join("switcher")).unwrap();
        storage::create_private_directory(&root.join("Claude-synthetic")).unwrap();
        storage::atomic_write(&root.join("Claude-synthetic/session"), b"synthetic-A").unwrap();
        Self(root)
    }
    fn coordinator(&self, environment: Box<dyn Environment>) -> MigrationCoordinator {
        coordinator(&self.0, environment)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn coordinator(root: &Path, environment: Box<dyn Environment>) -> MigrationCoordinator {
    MigrationCoordinator {
        root: root.join("switcher"),
        active: root.join("Claude-synthetic"),
        owner: unsafe { libc::geteuid() },
        environment,
    }
}
struct Stopped;
impl Environment for Stopped {
    fn quit(&self, _: u32) -> io::Result<()> {
        Ok(())
    }
    fn stopped(&self, _: u32) -> io::Result<()> {
        Ok(())
    }
}
struct HoldAccount(PathBuf);
impl Environment for HoldAccount {
    fn quit(&self, _: u32) -> io::Result<()> {
        Ok(())
    }
    fn stopped(&self, _: u32) -> io::Result<()> {
        storage::atomic_write(&self.0.join("account-ready"), b"ready")?;
        let deadline = Instant::now() + Duration::from_secs(15);
        while !self.0.join("release-account").exists() {
            assert!(Instant::now() < deadline, "account fixture timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn spawn(binary: &Path, test: &str, root: &Path, role: &str, edition: &str) -> ChildGuard {
    ChildGuard(
        Command::new(binary)
            .args(["--exact", test, "--nocapture"])
            .env("SWITCHER_UPDATE_CONTENTION_ROOT", root)
            .env("SWITCHER_UPDATE_CONTENTION_ROLE", role)
            .env("SWITCHER_UPDATE_CONTENTION_EDITION", edition)
            .stdin(Stdio::null())
            .spawn()
            .unwrap(),
    )
}
fn wait_ready(child: &mut ChildGuard, path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "fixture exited before ready"
        );
        assert!(Instant::now() < deadline, "fixture did not become ready");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn wait_success(child: &mut ChildGuard) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "fixture failed: {status}");
            return;
        }
        assert!(Instant::now() < deadline, "fixture did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn checked_root() -> PathBuf {
    let root = PathBuf::from(std::env::var_os("SWITCHER_UPDATE_CONTENTION_ROOT").unwrap());
    assert_eq!(
        root.parent(),
        Some(std::env::temp_dir().canonicalize().unwrap().as_path())
    );
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("switcher-entry-contention-")
    );
    storage::validate_no_symlinks(&root).unwrap();
    let metadata = fs::metadata(&root).unwrap();
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o700);
    root
}

#[test]
fn account_child() {
    if std::env::var_os("SWITCHER_UPDATE_CONTENTION_ROLE").as_deref()
        != Some(std::ffi::OsStr::new("account"))
    {
        return;
    }
    let root = checked_root();
    coordinator(&root, Box::new(HoldAccount(root.clone())))
        .select_a()
        .unwrap();
}

#[test]
#[ignore = "requires the separately built desktop test executable"]
fn account_and_update_entrypoints_exclude_each_other_across_processes() {
    let desktop = PathBuf::from(
        std::env::var_os("SWITCHER_UPDATE_FIXTURE_BIN").expect("desktop test executable required"),
    );
    let core = std::env::current_exe().unwrap();
    for edition in ["personal", "public"] {
        let fixture = Fixture::new();
        let account = fixture.coordinator(Box::new(Stopped));
        let mut initial_update = spawn(&desktop, UPDATE_CHILD, &fixture.0, "finish", edition);
        wait_success(&mut initial_update);
        let update_dir = fixture.0.join(format!("switcher/v2/{edition}"));
        let state_before = fs::read(update_dir.join("updates.json")).unwrap();
        let clock_before = fs::read(update_dir.join("clock.json")).unwrap();
        {
            let mut update = spawn(&desktop, UPDATE_CHILD, &fixture.0, "hold", edition);
            wait_ready(&mut update, &fixture.0.join("update-ready"));
            assert!(
                account
                    .select_b()
                    .unwrap_err()
                    .to_string()
                    .contains("其他账号操作正在进行")
            );
            assert_eq!(
                fs::read(fixture.0.join("Claude-synthetic/session")).unwrap(),
                b"synthetic-A"
            );
            assert!(!fixture.0.join("switcher/current").exists());
            assert_eq!(
                fs::read(update_dir.join("updates.json")).unwrap(),
                state_before
            );
            assert_eq!(
                fs::read(update_dir.join("clock.json")).unwrap(),
                clock_before
            );
            update.0.kill().unwrap();
            assert_eq!(update.0.wait().unwrap().signal(), Some(libc::SIGKILL));
        }
        assert_eq!(
            fs::read(update_dir.join("updates.json")).unwrap(),
            state_before
        );
        assert_eq!(
            fs::read(update_dir.join("clock.json")).unwrap(),
            clock_before
        );
        account.select_b().unwrap();
        assert_eq!(
            fs::read(fixture.0.join("switcher/current")).unwrap(),
            b"B\n"
        );
        assert_eq!(
            fs::read(fixture.0.join("switcher/accounts/A/session")).unwrap(),
            b"synthetic-A"
        );

        {
            let mut account_child = spawn(&core, ACCOUNT_CHILD, &fixture.0, "account", edition);
            wait_ready(&mut account_child, &fixture.0.join("account-ready"));
            let mut update = spawn(&desktop, UPDATE_CHILD, &fixture.0, "attempt", edition);
            wait_success(&mut update);
            assert_eq!(
                fs::read(fixture.0.join("switcher/current")).unwrap(),
                b"B\n"
            );
            assert_eq!(
                fs::read(update_dir.join("updates.json")).unwrap(),
                state_before
            );
            assert_eq!(
                fs::read(update_dir.join("clock.json")).unwrap(),
                clock_before
            );
            storage::atomic_write(&fixture.0.join("release-account"), b"ready").unwrap();
            wait_success(&mut account_child);
        }
        assert_eq!(
            fs::read(fixture.0.join("switcher/current")).unwrap(),
            b"A\n"
        );
        assert_eq!(
            fs::read(fixture.0.join("Claude-synthetic/session")).unwrap(),
            b"synthetic-A"
        );
        let mut update = spawn(&desktop, UPDATE_CHILD, &fixture.0, "finish", edition);
        wait_success(&mut update);
    }
}
