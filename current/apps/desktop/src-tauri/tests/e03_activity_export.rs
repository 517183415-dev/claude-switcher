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
