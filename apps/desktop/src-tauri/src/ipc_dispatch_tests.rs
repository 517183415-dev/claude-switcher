//! Real command decoding, ordinary ACL and the product registry on Tauri MockRuntime.
//! This does not exercise WKWebView, native Keychain or external publication.
use crate::bridge::DesktopState;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets},
    webview::InvokeRequest,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct SyntheticRoot(PathBuf);
impl SyntheticRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-command-dispatch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn app(&self) -> (tauri::App<MockRuntime>, tauri::WebviewWindow<MockRuntime>) {
        let state = DesktopState::isolated_activity_fixture(
            self.0.clone(),
            if cfg!(feature = "personal") {
                switcher_domain::Edition::Personal
            } else {
                switcher_domain::Edition::Public
            },
        );
        let mut context = mock_context(noop_assets());
        *context.runtime_authority_mut() = crate::ipc_policy_tests::ordinary_policy().0;
        let builder = mock_builder().manage(Arc::new(state));
        #[cfg(feature = "personal")]
        let builder = builder.manage(Arc::new(
            crate::publisher_recovery_commands::RecoverySessions::default(),
        ));
        let app = builder
            .invoke_handler(crate::ipc::handler())
            .build(context)
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        (app, window)
    }
    #[cfg(feature = "personal")]
    fn state_path(&self) -> PathBuf {
        let root = self.0.join("v2/personal/publisher");
        let active: Value =
            serde_json::from_slice(&fs::read(root.join("active.json")).unwrap()).unwrap();
        root.join(active["generation"].as_str().unwrap())
            .join("state.json")
    }
    #[cfg(feature = "personal")]
    fn persisted(&self, expected: &Value) {
        let path = self.state_path();
        let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(&stored["draft"], expected);
        assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
    }
}
impl Drop for SyntheticRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn invoke(
    window: &tauri::WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    invoke_from(window, command, body, "tauri://localhost/")
}
fn invoke_from(
    window: &tauri::WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
    url: &str,
) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}

#[test]
fn ordinary_registry_rejects_untrusted_contexts_and_missing_arguments_without_writes() {
    let root = SyntheticRoot::new();
    let (app, main) = root.app();
    let other = tauri::WebviewWindowBuilder::new(&app, "other", Default::default())
        .build()
        .unwrap();
    let personal = cfg!(feature = "personal");
    let registered = crate::ipc::registered_names(personal);
    assert_eq!(registered.len(), if personal { 48 } else { 23 });
    for command in &registered {
        for (window, url) in [
            (&other, "tauri://localhost/"),
            (&main, "https://example.invalid/"),
            (&main, "http://127.0.0.1:5555/"),
        ] {
            let error = invoke_from(window, command, json!({}), url).unwrap_err();
            assert!(
                error.as_str().unwrap().contains("not allowed"),
                "{command} {url}: {error}"
            );
        }
    }
    let mut denied = vec![
        "forbidden_probe".to_owned(),
        "finish_smoke".to_owned(),
        "__unregistered__".to_owned(),
    ];
    if !personal {
        denied.extend(
            crate::ipc::registered_names(true)
                .difference(&registered)
                .cloned(),
        );
    }
    for command in denied {
        let error = invoke(&main, &command, json!({})).unwrap_err();
        assert!(
            error.as_str().unwrap().contains("not allowed"),
            "{command}: {error}"
        );
    }
    let mut required = vec![
        "account_action",
        "preview_repair",
        "execute_repair",
        "save_preferences",
        "software_update",
        "open_ad_link",
        "ad_cached_image",
        "preview_region_change",
        "cancel_repair",
    ];
    if personal {
        required.extend(["ad_draft_save", "import_ad_image", "ad_image_preview"]);
    }
    for command in required {
        let error = invoke(&main, command, json!({})).unwrap_err();
        assert!(
            error.as_str().unwrap().contains("invalid args"),
            "{command}: {error}"
        );
    }
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    assert!(
        invoke(&main, "runtime_info", json!({})).is_ok(),
        "positive control"
    );
}

#[test]
fn ordinary_registry_preferences_and_redacted_export_persist_on_reopen() {
    use switcher_core::storage;
    let root = SyntheticRoot::new();
    let (app, main) = root.app();
    let edition = if cfg!(feature = "personal") {
        "personal"
    } else {
        "public"
    };
    assert_eq!(
        invoke(&main, "runtime_info", json!({})).unwrap()["edition"],
        edition
    );
    let mut preferences =
        serde_json::to_value(switcher_domain::desktop::Preferences::default()).unwrap();
    preferences["updates_paused_until"] = json!(1234567890);
    let saved = invoke(
        &main,
        "save_preferences",
        json!({"preferences":preferences}),
    )
    .unwrap();
    assert_eq!(saved, preferences);
    let directory = root.0.join("v2").join(edition);
    let path = directory.join("preferences.json");
    let before = fs::read(&path).unwrap();
    let disk: Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(disk, saved);
    let mut invalid = preferences.clone();
    invalid["ads"]["slots_enabled"] = json!([true, false, false]);
    invalid["ads"]["request_purpose_acknowledged"] = json!(false);
    assert!(invoke(&main, "save_preferences", json!({"preferences":invalid})).is_err());
    invalid = preferences;
    invalid["unexpected"] = json!(true);
    assert!(invoke(&main, "save_preferences", json!({"preferences":invalid})).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    let raw = "IP 203.0.113.9\nCookie: DISPATCH_SYNTHETIC_SECRET\nhttps://user:password@private.example/path";
    storage::atomic_json(
        &directory.join("activity.json"),
        &json!([
            {"observed_at":1,"level":"info","message":raw}
        ]),
    )
    .unwrap();
    let activity = invoke(&main, "activity_log", json!({})).unwrap();
    let exported = invoke(&main, "export_activity", json!({})).unwrap();
    let expected_path = directory.join("exports/activity-redacted.json");
    assert_eq!(PathBuf::from(exported[0].as_str().unwrap()), expected_path);
    assert_eq!(exported[1], activity);
    let bytes = fs::read(&expected_path).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), activity);
    let text = String::from_utf8(bytes).unwrap();
    for marker in [
        "203.0.113.9",
        "DISPATCH_SYNTHETIC_SECRET",
        "password",
        "private.example",
    ] {
        assert!(!text.contains(marker), "secret marker escaped redaction");
    }
    for file in [&path, &expected_path] {
        assert_eq!(
            fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(main);
    drop(app);
    let (_app, main) = root.app();
    assert_eq!(invoke(&main, "activity_log", json!({})).unwrap(), activity);
    assert_eq!(
        invoke(&main, "export_activity", json!({})).unwrap()[1],
        activity
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}
#[cfg(feature = "personal")]
fn read(window: &tauri::WebviewWindow<MockRuntime>) -> Value {
    invoke(window, "ad_draft_read", json!({})).unwrap()
}

#[cfg(feature = "personal")]
fn publisher_tree(
    root: &std::path::Path,
) -> std::collections::BTreeMap<PathBuf, (bool, u32, Vec<u8>)> {
    fn collect(
        root: &std::path::Path,
        directory: &std::path::Path,
        files: &mut std::collections::BTreeMap<PathBuf, (bool, u32, Vec<u8>)>,
    ) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.is_symlink());
            let is_directory = metadata.is_dir();
            let bytes = if is_directory {
                Vec::new()
            } else {
                fs::read(&path).unwrap()
            };
            files.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (is_directory, metadata.permissions().mode(), bytes),
            );
            if is_directory {
                collect(root, &path, files);
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    collect(root, root, &mut files);
    files
}

#[test]
#[cfg(feature = "personal")]
fn ordinary_dispatch_recovery_rejects_invalid_backups_without_changing_publisher_tree() {
    use crate::publisher::backup;
    let root = SyntheticRoot::new();
    let (app, main) = root.app();
    read(&main);
    let imported = invoke(&main, "import_ad_image", json!({"bytes":include_bytes!("../../tests/native-ipc-business/backup-source.png").as_slice()})).unwrap();
    let draft = invoke(&main, "pause_ad", json!({"slot":"residential_broadband"})).unwrap();
    let directory = root.0.join("v2/personal/publisher");
    let before = publisher_tree(&directory);
    assert!(
        before
            .keys()
            .any(|p| p.file_name().unwrap() == imported["asset"]["sha256"].as_str().unwrap())
    );
    let passphrase = "synthetic-backup-passphrase";
    let sealed = backup::seal(b"{}", passphrase.as_bytes()).unwrap();
    assert_eq!(
        backup::open(&sealed, passphrase.as_bytes())
            .unwrap()
            .as_slice(),
        b"{}"
    );
    let mut bad_magic = sealed.clone();
    bad_magic[0] ^= 1;
    let mut bad_kdf = sealed.clone();
    bad_kdf[8] ^= 1;
    let mut tampered = sealed.clone();
    *tampered.last_mut().unwrap() ^= 1;
    let cases = [
        (Vec::new(), passphrase, "backup_header"),
        (bad_magic, passphrase, "backup_header"),
        (sealed[..75].to_vec(), passphrase, "backup_header"),
        (bad_kdf, passphrase, "backup_kdf_parameters"),
        (
            sealed.clone(),
            "synthetic-wrong-passphrase",
            "backup_password_or_integrity",
        ),
        (sealed.clone(), "short", "backup_passphrase_length"),
        (tampered, passphrase, "backup_password_or_integrity"),
        (sealed, passphrase, "payload_json"),
    ];
    for command in ["restore_publisher_backup", "publisher_archive_import"] {
        for (bytes, password, expected) in &cases {
            let error =
                invoke(&main, command, json!({"bytes":bytes,"passphrase":password})).unwrap_err();
            assert!(
                error.as_str().unwrap().contains(expected),
                "{command}: expected {expected}, got {error}"
            );
            assert_eq!(publisher_tree(&directory), before, "{command}: {expected}");
            assert_eq!(read(&main), draft);
        }
    }
    drop(main);
    drop(app);
    let (_app, main) = root.app();
    assert_eq!(read(&main), draft);
    assert_eq!(publisher_tree(&directory), before);
}

#[test]
#[cfg(feature = "personal")]
fn ordinary_dispatch_six_campaign_edits_pauses_and_reopen_keep_committed_draft() {
    let root = SyntheticRoot::new();
    let (app, window) = root.app();
    let initial = read(&window);
    let image = invoke(
        &window,
        "import_ad_image",
        json!({
            "bytes": include_bytes!("../../tests/native-ipc-business/backup-source.png").as_slice()
        }),
    )
    .unwrap();
    let preview = invoke(
        &window,
        "ad_image_preview",
        json!({"sha256": image["asset"]["sha256"]}),
    )
    .unwrap();
    assert_eq!(preview, image["preview_data_url"]);
    assert!(
        preview
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    let mut draft = initial.clone();
    let now = switcher_core::checks::unix_now();
    for (i, slot) in draft["slots"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        slot["campaigns"] = Value::Array((0..2).map(|j| json!({
            "campaign_id": format!("synthetic-{i}-{j}"), "merchant_id": "synthetic-merchant",
            "merchant_name": "合成商家", "title": format!("合成广告 {i}/{j}"), "description": "仅本机测试",
            "image": image["asset"], "button_text": "查看", "target_url": "https://synthetic.example.com/",
            "starts_at": now + j * 3600, "ends_at": now + (j + 1) * 3600, "enabled": true
        })).collect());
    }
    let saved = invoke(&window, "ad_draft_save", json!({"manifest":draft})).unwrap();
    assert_eq!(saved["slots"], draft["slots"]);
    assert!(
        saved["content_revision"].as_u64().unwrap() > initial["content_revision"].as_u64().unwrap()
    );
    root.persisted(&saved);
    drop(window);
    drop(app);
    let (_app, window) = root.app();
    assert_eq!(read(&window), saved);
    let before = fs::read(root.state_path()).unwrap();
    assert!(
        invoke(&window, "ad_draft_save", json!({}))
            .unwrap_err()
            .as_str()
            .unwrap()
            .contains("invalid args")
    );
    assert_eq!(fs::read(root.state_path()).unwrap(), before);
    let mut latest = saved;
    for i in 0..3 {
        let mut edit = latest.clone();
        edit["slots"][i]["campaigns"][1]["title"] = json!(format!("修改后的合成广告 {i}"));
        let changed = invoke(&window, "ad_draft_save", json!({"manifest":edit})).unwrap();
        assert_eq!(changed["slots"], edit["slots"]);
        let mut invalid = changed.clone();
        invalid["slots"][i]["campaigns"][1]["starts_at"] =
            invalid["slots"][i]["campaigns"][0]["starts_at"].clone();
        let bytes = fs::read(root.state_path()).unwrap();
        assert!(invoke(&window, "ad_draft_save", json!({"manifest":invalid})).is_err());
        assert_eq!(fs::read(root.state_path()).unwrap(), bytes);
        assert_eq!(read(&window), changed);
        latest = invoke(
            &window,
            "pause_ad",
            json!({"slot":changed["slots"][i]["slot"]}),
        )
        .unwrap();
        let mut expected = changed["slots"].clone();
        expected[i]["enabled"] = json!(false);
        assert_eq!(latest["slots"], expected);
        root.persisted(&latest);
    }
    let all = invoke(&window, "pause_ad", json!({"slot":null})).unwrap();
    for slot in all["slots"].as_array().unwrap() {
        assert_eq!(slot["enabled"], false);
        assert!(slot["campaigns"].as_array().unwrap().is_empty());
    }
    root.persisted(&all);
    assert_eq!(
        invoke(&window, "publisher_status", json!({})).unwrap()["status"],
        "wait_signing_key"
    );
}

#[test]
#[cfg(feature = "personal")]
fn ordinary_dispatch_wrong_window_and_corrupt_state_never_reset_draft() {
    let root = SyntheticRoot::new();
    let (app, main) = root.app();
    let saved = read(&main);
    let state = root.state_path();
    let bytes = fs::read(&state).unwrap();
    let other = tauri::WebviewWindowBuilder::new(&app, "other", Default::default())
        .build()
        .unwrap();
    for command in [
        "ad_draft_read",
        "ad_draft_save",
        "pause_ad",
        "import_ad_image",
        "ad_image_preview",
        "publisher_status",
    ] {
        let error = invoke(&other, command, json!({})).unwrap_err();
        assert!(
            error.as_str().unwrap().contains("not allowed"),
            "{command}: {error}"
        );
    }
    assert_eq!(fs::read(&state).unwrap(), bytes);
    assert_eq!(read(&main), saved);
    fs::write(&state, b"{invalid synthetic state").unwrap();
    for command in ["ad_draft_read", "publisher_status", "pause_ad"] {
        let args = if command == "pause_ad" {
            json!({"slot":null})
        } else {
            json!({})
        };
        assert!(invoke(&main, command, args).is_err());
        assert_eq!(fs::read(&state).unwrap(), b"{invalid synthetic state");
    }
}
