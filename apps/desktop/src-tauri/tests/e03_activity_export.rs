//! Actual fixed-path export under isolated directories; no HOME or native UI writes.
#![allow(dead_code)]
#[path = "../src/bridge.rs"]
mod bridge;
#[path = "../src/update_runtime.rs"]
mod update_runtime;

use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use switcher_core::storage;
use switcher_domain::Edition;

fn isolated(suffix: &str) -> bridge::DesktopState {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "switcher-e03-export-{}-{suffix}",
        std::process::id()
    ));
    storage::create_private_directory(&root).unwrap();
    let mut state = bridge::DesktopState::new(Edition::Personal).unwrap();
    state.root = root;
    state
}

#[test]
fn concurrent_successful_activity_writes_preserve_each_event() {
    use std::sync::{Arc, Barrier};

    const WRITERS: usize = 32;
    const READERS: usize = 4;
    let state = isolated("concurrent");
    let root = state.root.clone();
    for edition in [Edition::Personal, Edition::Public] {
        let state = bridge::DesktopState::isolated_activity_fixture(root.clone(), edition);
        state.prepare_directory().unwrap();
        let start = Arc::new(Barrier::new(WRITERS + READERS));
        let errors = std::thread::scope(|scope| {
            let mut operations: Vec<_> = (0..WRITERS)
                .map(|index| {
                    let start = Arc::clone(&start);
                    let state = &state;
                    scope.spawn(move || {
                        start.wait();
                        state.log("info", &format!("concurrent-event-{index}"))
                    })
                })
                .collect();
            for _ in 0..READERS {
                let start = Arc::clone(&start);
                let state = &state;
                operations.push(scope.spawn(move || {
                    start.wait();
                    for _ in 0..8 {
                        state.activity()?;
                    }
                    Ok(())
                }));
            }
            operations
                .into_iter()
                .filter_map(|operation| operation.join().unwrap().err())
                .collect::<Vec<_>>()
        });
        let entries = state.activity().unwrap();
        let missing: Vec<_> = (0..WRITERS)
            .filter(|index| {
                !entries
                    .iter()
                    .any(|entry| entry.message == format!("concurrent-event-{index}"))
            })
            .collect();
        assert!(
            errors.is_empty() && missing.is_empty(),
            "{edition:?}: {} errors, missing {missing:?}; {} records persisted",
            errors.len(),
            entries.len()
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_activity_read_does_not_create_a_directory_or_lock() {
    let state = isolated("empty-read");
    assert!(state.activity().unwrap().is_empty());
    assert!(!state.root.join("v2").exists());
    state.prepare_directory().unwrap();
    assert!(state.activity().unwrap().is_empty());
    assert!(!state.directory().join("activity.lock").exists());
    std::fs::remove_dir_all(state.root).unwrap();
}

#[test]
fn unsafe_activity_lock_is_rejected_without_changing_data() {
    let state = isolated("unsafe-lock");
    state.log("info", "existing event").unwrap();
    let activity = state.directory().join("activity.json");
    let before = std::fs::read(&activity).unwrap();
    let lock = state.directory().join("activity.lock");
    let sentinel = state.root.join("sentinel");
    storage::atomic_write(&sentinel, b"sentinel").unwrap();
    std::fs::remove_file(&lock).unwrap();

    for hard_link in [false, true] {
        if hard_link {
            std::fs::hard_link(&sentinel, &lock).unwrap();
        } else {
            symlink(&sentinel, &lock).unwrap();
        }
        assert!(state.log("info", "must not persist").is_err());
        assert!(state.activity().is_err());
        assert_eq!(std::fs::read(&activity).unwrap(), before);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");
        std::fs::remove_file(&lock).unwrap();
    }
    std::fs::remove_dir_all(state.root).unwrap();
}

#[test]
fn busy_activity_lock_returns_an_error_and_allows_retry() {
    let state = isolated("busy-lock");
    state.log("info", "existing event").unwrap();
    let activity = state.directory().join("activity.json");
    let before = std::fs::read(&activity).unwrap();
    let lock = storage::ActivityLogLock::acquire(&state.directory()).unwrap();
    let started = std::time::Instant::now();
    assert_eq!(
        state.log("info", "retry event").unwrap_err(),
        "活动记录忙，请稍后重试"
    );
    assert!(started.elapsed() >= std::time::Duration::from_secs(2));
    assert_eq!(std::fs::read(&activity).unwrap(), before);
    drop(lock);
    state.log("info", "retry event").unwrap();
    assert_eq!(state.activity().unwrap().len(), 2);
    std::fs::remove_dir_all(state.root).unwrap();
}

// Invoked only by the parent test, with an isolated synthetic root.
#[test]
#[ignore = "subprocess fixture"]
fn activity_lock_child_fixture() {
    let root = std::path::PathBuf::from(std::env::var_os("SWITCHER_TEST_ACTIVITY_ROOT").unwrap());
    let edition = match std::env::var("SWITCHER_TEST_ACTIVITY_EDITION")
        .unwrap()
        .as_str()
    {
        "personal" => Edition::Personal,
        "public" => Edition::Public,
        _ => panic!("unexpected edition"),
    };
    let state = bridge::DesktopState::isolated_activity_fixture(root.clone(), edition);
    assert!(matches!(
        storage::ActivityLogLock::try_acquire(&state.directory()),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
    std::fs::write(root.join("child-ready"), b"ready").unwrap();
    state.log("info", "child event").unwrap();
}

#[test]
fn activity_lock_serializes_separate_processes() {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };

    let state = isolated("processes");
    let root = state.root.clone();
    for edition in [Edition::Personal, Edition::Public] {
        let state = bridge::DesktopState::isolated_activity_fixture(root.clone(), edition);
        state.log("info", "parent event").unwrap();
        let before = std::fs::read(state.directory().join("activity.json")).unwrap();
        let lock = storage::ActivityLogLock::acquire(&state.directory()).unwrap();
        let ready = root.join("child-ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "activity_lock_child_fixture",
                "--exact",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .env("HOME", &root)
            .env("TMPDIR", &root)
            .env("SWITCHER_TEST_ACTIVITY_ROOT", &root)
            .env(
                "SWITCHER_TEST_ACTIVITY_EDITION",
                bridge::edition_name(edition),
            )
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() && Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !ready.exists() {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "child did not observe the held lock: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert_eq!(
            std::fs::read(state.directory().join("activity.json")).unwrap(),
            before
        );
        drop(lock);
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let events = state.activity().unwrap();
        assert!(events.iter().any(|entry| entry.message == "parent event"));
        assert!(events.iter().any(|entry| entry.message == "child event"));
        std::fs::remove_file(ready).unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn multiline_sensitive_activity_is_hidden_in_storage_preview_and_export() {
    let mut state = isolated("multiline-privacy");
    let values = ["E07_MULTILINE_A_5426", "E07_MULTILINE_B_1924"];
    let messages = [
        "开始\nAuthorization:\n  Bearer E07_MULTILINE_A_5426\n完成",
        "开始\n{\n  \"token\": {\n    \"value\": \"E07_MULTILINE_B_1924\"\n  }\n}",
    ];
    for edition in [Edition::Personal, Edition::Public] {
        state.edition = edition;
        for message in messages {
            state.log("info", message).unwrap();
        }
        let persisted: serde_json::Value =
            storage::read_json(&state.directory().join("activity.json")).unwrap();
        let preview = state.activity().unwrap();
        let (path, exported) = state.export_activity().unwrap();
        let exported_file: serde_json::Value =
            storage::read_json(std::path::Path::new(&path)).unwrap();
        assert_eq!(persisted, serde_json::to_value(&preview).unwrap());
        assert_eq!(exported_file, serde_json::to_value(&preview).unwrap());
        assert_eq!(serde_json::to_value(exported).unwrap(), exported_file);
        assert_eq!(preview.len(), messages.len());
        for entry in &preview {
            assert!(entry.message.starts_with("开始\n"));
        }
        let visible = exported_file.to_string();
        for value in values {
            assert!(!visible.contains(value));
        }

        // Previously persisted raw entries must be hidden on read and export.
        storage::atomic_json(
            &state.directory().join("activity.json"),
            &serde_json::json!([{"observed_at":1,"level":"info","message":messages[1]}]),
        )
        .unwrap();
        let preview = state.activity().unwrap();
        let (path, exported) = state.export_activity().unwrap();
        let exported_file: serde_json::Value =
            storage::read_json(std::path::Path::new(&path)).unwrap();
        assert_eq!(serde_json::to_value(&preview).unwrap(), exported_file);
        assert_eq!(serde_json::to_value(exported).unwrap(), exported_file);
        assert!(!exported_file.to_string().contains(values[1]));
        // A later write also replaces the old stored entry with its redacted form.
        state.log("info", "后续事件").unwrap();
        let persisted: serde_json::Value =
            storage::read_json(&state.directory().join("activity.json")).unwrap();
        assert!(!persisted.to_string().contains(values[1]));
        let entries = state.activity().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].message, "后续事件");
    }
    std::fs::remove_dir_all(state.root).unwrap();
}

#[test]
fn export_uses_the_same_redacted_snapshot_and_private_edition_path() {
    let mut state = isolated("content");
    for edition in [Edition::Personal, Edition::Public] {
        state.edition = edition;
        state.prepare_directory().unwrap();
        // Include historical unredacted input; export must apply the normal read rule.
        let raw = "IP 203.0.113.9\nCookie: E03_SYNTHETIC_SECRET_98d11\nhttps://user:password@private.example/path";
        storage::atomic_json(
            &state.directory().join("activity.json"),
            &serde_json::json!([{"observed_at":1,"level":"info","message":raw}]),
        )
        .unwrap();
        let (path, snapshot) = state.export_activity().unwrap();
        let expected_path = state.directory().join("exports/activity-redacted.json");
        assert_eq!(std::path::Path::new(&path), expected_path);
        let bytes = storage::read_private_bytes(&expected_path, 65536).unwrap();
        let exported: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(exported, serde_json::to_value(&snapshot).unwrap());
        assert_eq!(
            exported,
            serde_json::to_value(state.activity().unwrap()).unwrap()
        );
        assert_eq!(snapshot[0].message, switcher_core::diagnostics::redact(raw));
        let text = String::from_utf8(bytes).unwrap();
        for secret in [
            "203.0.113.9",
            "E03_SYNTHETIC_SECRET_98d11",
            "password",
            "private.example",
        ] {
            assert!(!text.contains(secret));
        }
        assert_eq!(
            std::fs::metadata(expected_path.parent().unwrap())
                .unwrap()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&expected_path).unwrap().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::metadata(&expected_path).unwrap().nlink(), 1);
        // A later export replaces old data, without appending a stale export event.
        state.log("info", "后续脱敏事件").unwrap();
        let (_, next) = state.export_activity().unwrap();
        assert_eq!(next.len(), 2);
        let actual: serde_json::Value = storage::read_json(&expected_path).unwrap();
        assert_eq!(actual, serde_json::to_value(next).unwrap());
    }
    std::fs::remove_dir_all(state.root).unwrap();
}

#[test]
fn export_rejects_links_and_nonprivate_directory_without_changing_targets() {
    let state = isolated("unsafe");
    state.export_activity().unwrap();
    let exports = state.directory().join("exports");
    let destination = exports.join("activity-redacted.json");
    let untouched = state.root.join("untouched.json");
    storage::atomic_json(&untouched, &vec!["sentinel"]).unwrap();
    let before = std::fs::read(&untouched).unwrap();
    std::fs::remove_file(&destination).unwrap();
    symlink(&untouched, &destination).unwrap();
    assert!(state.export_activity().is_err());
    assert_eq!(std::fs::read(&untouched).unwrap(), before);
    std::fs::remove_file(&destination).unwrap();
    std::fs::hard_link(&untouched, &destination).unwrap();
    assert!(state.export_activity().is_err());
    assert_eq!(std::fs::read(&untouched).unwrap(), before);
    std::fs::remove_file(&destination).unwrap();
    std::fs::set_permissions(&exports, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(state.export_activity().is_err());
    std::fs::set_permissions(&exports, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir(&exports).unwrap();
    let outside = state.root.join("outside");
    storage::create_private_directory(&outside).unwrap();
    symlink(&outside, &exports).unwrap();
    assert!(state.export_activity().is_err());
    assert!(!outside.join("activity-redacted.json").exists());
    assert_eq!(std::fs::read(&untouched).unwrap(), before);
    std::fs::remove_dir_all(state.root).unwrap();
}
