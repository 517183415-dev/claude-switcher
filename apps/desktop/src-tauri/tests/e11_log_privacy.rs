//! Exercise actual desktop log persistence and preview under an isolated HOME.
#![allow(dead_code)]
#[path = "../src/bridge.rs"]
mod bridge;
#[path = "../src/update_runtime.rs"]
mod update_runtime;

#[test]
fn desktop_log_privacy_roundtrip() {
    if std::env::var_os("SWITCHER_E11_LOG_CHILD").is_some() {
        let state = bridge::DesktopState::new(switcher_domain::Edition::Personal).unwrap();
        let raw = "出口203.0.113.9 [2001:db8::4]\nCookie: E11_SYNTHETIC_LOGIN_COOKIE_9ef0721a\nhttps://user:E11_SYNTHETIC_ACCOUNT_A_7df3502b@private.example:443/path";
        let expected = switcher_core::diagnostics::redact(raw);
        state.log("info", raw).unwrap();
        let persisted = switcher_core::storage::read_private_bytes(
            &state.directory().join("activity.json"),
            65536,
        )
        .unwrap();
        let preview = state.activity().unwrap();
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0].message, expected);
        // App.svelte exports this same activity array with JSON.stringify.
        let exported = serde_json::to_vec_pretty(&preview).unwrap();
        let stored: serde_json::Value = serde_json::from_slice(&persisted).unwrap();
        let output: serde_json::Value = serde_json::from_slice(&exported).unwrap();
        assert_eq!(stored, output);
        for bytes in [&persisted, &exported] {
            let text = String::from_utf8_lossy(bytes);
            for secret in [
                "203.0.113.9",
                "2001:db8::4",
                "E11_SYNTHETIC_LOGIN_COOKIE_9ef0721a",
                "E11_SYNTHETIC_ACCOUNT_A_7df3502b",
                "private.example",
            ] {
                assert!(!text.contains(secret));
            }
        }
        // A historical unredacted message also receives the shared rule on read.
        let entries = serde_json::json!([{"observed_at":1,"level":"info","message":raw}]);
        switcher_core::storage::atomic_json(&state.directory().join("activity.json"), &entries)
            .unwrap();
        assert_eq!(state.activity().unwrap()[0].message, expected);
        return;
    }
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("switcher-e11-desktop-log-{}", std::process::id()));
    switcher_core::storage::create_private_directory(&root).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "desktop_log_privacy_roundtrip", "--nocapture"])
        .env("SWITCHER_E11_LOG_CHILD", "1")
        .env("HOME", &root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "isolated desktop log test failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn desktop_personal_background_update_actions_are_denied_before_transport() {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "switcher-e11-desktop-update-{}",
        std::process::id()
    ));
    let runtime =
        update_runtime::UpdateRuntime::new(root.clone(), switcher_domain::Edition::Personal)
            .with_clock(|| {
                Ok(switcher_core::time::ClockSample {
                    utc_secs: 1_700_000_000,
                    continuous_ms: 1_700_000_000_000,
                    boot_id: "test-boot".into(),
                })
            });
    for action in [
        switcher_core::updates::UpdateAction::Check,
        switcher_core::updates::UpdateAction::Download,
        switcher_core::updates::UpdateAction::Install,
    ] {
        let result = runtime.run_task(action, false);
        assert!(!result.completed);
        assert_eq!(
            result.recovery.as_deref(),
            Some("personal_requires_user_action")
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unacknowledged_ad_preferences_are_rejected_without_persistence() {
    if std::env::var_os("SWITCHER_E11_AD_PREFS_CHILD").is_some() {
        use std::os::unix::fs::MetadataExt;
        for edition in [
            switcher_domain::Edition::Personal,
            switcher_domain::Edition::Public,
        ] {
            let state = bridge::DesktopState::new(edition).unwrap();
            let path = state.directory().join("preferences.json");
            for existing in [false, true] {
                let before = if existing {
                    state
                        .save_preferences(switcher_domain::desktop::Preferences::default())
                        .unwrap();
                    let metadata = std::fs::symlink_metadata(&path).unwrap();
                    Some((
                        std::fs::read(&path).unwrap(),
                        metadata.ino(),
                        metadata.mtime(),
                        metadata.mtime_nsec(),
                    ))
                } else {
                    None
                };
                for slot in 0..3 {
                    let mut preferences = switcher_domain::desktop::Preferences::default();
                    preferences.ads.slots_enabled[slot] = true;
                    assert_eq!(
                        state.save_preferences(preferences).unwrap_err(),
                        "请先确认广告请求用途，再开启广告位"
                    );
                    if let Some(expected) = &before {
                        let metadata = std::fs::symlink_metadata(&path).unwrap();
                        assert_eq!(
                            &(
                                std::fs::read(&path).unwrap(),
                                metadata.ino(),
                                metadata.mtime(),
                                metadata.mtime_nsec()
                            ),
                            expected
                        );
                    } else {
                        assert!(!path.exists());
                        assert!(!state.directory().exists());
                    }
                }
            }
        }
        return;
    }
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "switcher-e11-ad-preferences-{}",
        std::process::id()
    ));
    switcher_core::storage::create_private_directory(&root).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "unacknowledged_ad_preferences_are_rejected_without_persistence",
            "--nocapture",
        ])
        .env("SWITCHER_E11_AD_PREFS_CHILD", "1")
        .env("HOME", &root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "isolated preferences test failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn activity_snapshot(path: &std::path::Path) -> (Vec<u8>, u64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(path).unwrap();
    (
        std::fs::read(path).unwrap(),
        metadata.ino(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn preference_fault_child(test: &str, mode: &str, root: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture"])
        .env("SWITCHER_E11_PREF_FAULT", mode)
        .env("HOME", root);
    command
}

fn assert_fault_child(output: std::process::Output) {
    assert!(
        output.status.success(),
        "isolated preference fault test failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn blocked_preferences_destination_preserves_existing_activity_both_editions() {
    const TEST: &str = "blocked_preferences_destination_preserves_existing_activity_both_editions";
    if std::env::var_os("SWITCHER_E11_PREF_FAULT").is_some() {
        use std::os::unix::fs::MetadataExt;
        for edition in [
            switcher_domain::Edition::Personal,
            switcher_domain::Edition::Public,
        ] {
            let state = bridge::DesktopState::new(edition).unwrap();
            state.log("info", "synthetic existing activity").unwrap();
            let activity = state.directory().join("activity.json");
            let before = activity_snapshot(&activity);
            let preferences = state.directory().join("preferences.json");
            switcher_core::storage::create_private_directory(&preferences).unwrap();
            let inode = std::fs::symlink_metadata(&preferences).unwrap().ino();
            assert_eq!(
                state
                    .save_preferences(switcher_domain::desktop::Preferences::default())
                    .unwrap_err(),
                "设置保存失败"
            );
            assert_eq!(activity_snapshot(&activity), before);
            assert!(preferences.is_dir());
            assert_eq!(
                std::fs::symlink_metadata(&preferences).unwrap().ino(),
                inode
            );
            assert_eq!(std::fs::read_dir(&preferences).unwrap().count(), 0);
        }
        return;
    }
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "switcher-e11-blocked-preferences-{}",
        std::process::id()
    ));
    switcher_core::storage::create_private_directory(&root).unwrap();
    let output = preference_fault_child(TEST, "blocked", &root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert_fault_child(output);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires the dedicated single-layer account-free Seatbelt runner"]
fn activity_write_denial_returns_error_after_preferences_commit_both_editions() {
    const TEST: &str = "activity_write_denial_returns_error_after_preferences_commit_both_editions";
    if let Ok(mode) = std::env::var("SWITCHER_E11_PREF_FAULT") {
        for edition in [
            switcher_domain::Edition::Personal,
            switcher_domain::Edition::Public,
        ] {
            let state = bridge::DesktopState::new(edition).unwrap();
            let changed = switcher_domain::desktop::Preferences {
                updates_paused_until: Some(1_800_000_123),
                ..Default::default()
            };
            if mode == "prepare" {
                state
                    .save_preferences(switcher_domain::desktop::Preferences::default())
                    .unwrap();
            } else if mode == "denied" {
                let activity = state.directory().join("activity.json");
                let before = activity_snapshot(&activity);
                assert_eq!(
                    state.save_preferences(changed.clone()).unwrap_err(),
                    "活动记录保存失败"
                );
                assert_eq!(activity_snapshot(&activity), before);
                let reopened = bridge::DesktopState::new(edition).unwrap();
                assert_eq!(
                    serde_json::to_value(reopened.preferences().unwrap()).unwrap(),
                    serde_json::to_value(changed).unwrap()
                );
            } else {
                assert_eq!(mode, "reopen");
                assert_eq!(
                    serde_json::to_value(state.preferences().unwrap()).unwrap(),
                    serde_json::to_value(changed).unwrap()
                );
                assert_eq!(state.activity().unwrap().len(), 1);
            }
        }
        return;
    }
    panic!("run via the dedicated single-layer account-free Seatbelt runner");
}
