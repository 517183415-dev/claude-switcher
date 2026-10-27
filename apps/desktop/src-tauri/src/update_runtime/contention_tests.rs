//! Update-process fixture used by the account coordinator's cross-process test.
use super::*;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    time::{Duration, Instant},
};
use switcher_core::storage;

fn fixture_root() -> PathBuf {
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
fn clock() -> Result<ClockSample, &'static str> {
    Ok(ClockSample {
        utc_secs: 1_700_000_000,
        continuous_ms: 1_700_000_000_000,
        boot_id: "test-boot".into(),
    })
}
fn blocked_clock() -> Result<ClockSample, &'static str> {
    let root = fixture_root();
    storage::atomic_write(&root.join("update-ready"), b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !root.join("release-update").exists() {
        assert!(Instant::now() < deadline, "update fixture timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    clock()
}

#[test]
fn child_fixture() {
    let Some(role) = std::env::var_os("SWITCHER_UPDATE_CONTENTION_ROLE") else {
        return;
    };
    let root = fixture_root();
    let edition = match std::env::var("SWITCHER_UPDATE_CONTENTION_EDITION")
        .unwrap()
        .as_str()
    {
        "personal" => Edition::Personal,
        "public" => Edition::Public,
        _ => panic!("unexpected edition"),
    };
    let runtime = UpdateRuntime::new(root.join("switcher"), edition);
    match role.to_str().unwrap() {
        "hold" => assert!(
            runtime
                .with_clock(blocked_clock)
                .act(UpdateAction::Pause)
                .completed
        ),
        "attempt" => {
            let runtime = runtime.with_clock(clock);
            for result in [
                runtime.act(UpdateAction::Pause),
                runtime.run_task(updates::UpdateAction::Check, true),
            ] {
                assert!(!result.completed);
                assert_eq!(result.recovery.as_deref(), Some("lifecycle_busy"));
            }
            assert_eq!(runtime.snapshot().state, "lifecycle_busy");
        }
        "finish" => {
            let runtime = runtime.with_clock(clock);
            assert!(runtime.act(UpdateAction::Pause).completed);
            assert_eq!(runtime.snapshot().paused, Some(true));
        }
        _ => panic!("unexpected fixture role"),
    }
}
