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
        self.build_app(
            state,
            #[cfg(feature = "personal")]
            Arc::new(crate::publisher::security::ManagedVault::default()),
        )
    }
    fn build_app(
        &self,
        state: DesktopState,
        #[cfg(feature = "personal")] vault: Arc<crate::publisher::security::ManagedVault>,
    ) -> (tauri::App<MockRuntime>, tauri::WebviewWindow<MockRuntime>) {
        let mut context = mock_context(noop_assets());
        *context.runtime_authority_mut() = crate::ipc_policy_tests::ordinary_policy().0;
        let builder = mock_builder().manage(Arc::new(state));
        #[cfg(feature = "personal")]
        let builder = builder.manage(vault).manage(Arc::new(
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
    fn app_with_vault(
        &self,
        vault: Arc<crate::publisher::security::ManagedVault>,
    ) -> (tauri::App<MockRuntime>, tauri::WebviewWindow<MockRuntime>) {
        let state = DesktopState::isolated_activity_fixture(
            self.0.clone(),
            switcher_domain::Edition::Personal,
        );
        self.build_app(state, vault)
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

const SYNTHETIC_AD_SOURCE: [Option<&str>; 4] = [
    Some("https://ads.example.com/"),
    Some("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
    Some("synthetic-test-key"),
    Some("production"),
];

fn synthetic_ad_source_configured(materials: [Option<&str>; 4]) -> Result<bool, &'static str> {
    match materials {
        [None, None, None, None] => Ok(false),
        valid if valid == SYNTHETIC_AD_SOURCE => Ok(true),
        _ => Err("snapshot test requires empty or designated synthetic release materials"),
    }
}

#[test]
fn synthetic_ad_source_materials_reject_unexpected_inputs() {
    let valid = SYNTHETIC_AD_SOURCE;
    assert_eq!(synthetic_ad_source_configured([None; 4]), Ok(false));
    assert_eq!(synthetic_ad_source_configured(valid), Ok(true));
    let mut partial = valid;
    partial[1] = None;
    assert!(synthetic_ad_source_configured(partial).is_err());
    let mut unexpected = valid;
    unexpected[0] = Some("https://other.example.com/");
    assert!(synthetic_ad_source_configured(unexpected).is_err());
}

#[test]
fn workspace_snapshot_reports_the_selected_ad_source() {
    use switcher_core::{
        ad_runtime::{self, local::LocalTrust},
        storage,
    };
    use switcher_domain::Edition;

    // The configured case uses a public test vector and a reserved example
    // hostname. Network access is not needed for a workspace snapshot.
    const KEY: [u8; 32] = [
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07,
        0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07,
        0x51, 0x1a,
    ];
    let configured = synthetic_ad_source_configured([
        option_env!("SWITCHER_AD_SOURCE"),
        option_env!("SWITCHER_AD_PUBLIC_KEY_HEX"),
        option_env!("SWITCHER_AD_KEY_ID"),
        option_env!("SWITCHER_AD_CHANNEL"),
    ])
    .expect("unsupported release materials for snapshot test");
    if configured {
        assert!(ad_runtime::validate_public_release_materials().is_ok());
    }
    for edition in [Edition::Personal, Edition::Public] {
        let root = SyntheticRoot::new();
        let state = DesktopState::isolated_activity_fixture(root.0.clone(), edition);
        let snapshot = state.snapshot().unwrap();
        let missing = |messages: &[String]| {
            messages.iter().any(|message| {
                message.contains("广告内容来源尚未配置") || message.contains("远程广告上线未配置")
            })
        };
        assert_eq!(missing(&snapshot.pending_materials), !configured);
        assert!(snapshot.ads_manifest.is_none());

        let directory = ad_runtime::local::directory(&root.0);
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let trust_path = directory.join("trust.json");
        let mut trust = LocalTrust {
            schema: 1,
            key_id: "ads-personal-v1".into(),
            public_key: KEY,
        };
        storage::atomic_json(&trust_path, &trust).unwrap();
        let snapshot = state.snapshot().unwrap();
        assert!(!missing(&snapshot.pending_materials));
        assert!(snapshot.ads_manifest.is_none());

        trust.schema = 2;
        storage::atomic_json(&trust_path, &trust).unwrap();
        let snapshot = state.snapshot().unwrap();
        let uses_invalid_local = edition == Edition::Personal || !configured;
        assert_eq!(
            snapshot
                .pending_materials
                .iter()
                .any(|message| message.contains("local_ad_trust_scope")),
            uses_invalid_local
        );
        assert!(!missing(&snapshot.pending_materials));
        assert!(snapshot.ads_manifest.is_none());
    }
}

#[test]
fn ad_refresh_guidance_matches_disabled_and_missing_source() {
    use switcher_domain::Edition;

    for edition in [Edition::Personal, Edition::Public] {
        let disabled = crate::bridge::ad_refresh_waiting("ads_disabled", edition);
        assert!(!disabled.completed);
        assert!(disabled.message.contains("广告位已关闭"));
        let guidance = disabled.recovery.unwrap();
        assert!(guidance.contains("设置") && guidance.contains("开启"));
        assert!(!guidance.contains("Personal"));

        let missing = crate::bridge::ad_refresh_waiting("wait_ad_content_materials", edition);
        assert!(!missing.completed);
        assert!(missing.message.contains("来源") && missing.message.contains("未配置"));
        let guidance = missing.recovery.unwrap();
        assert!(guidance.contains(match edition {
            Edition::Personal => "本机",
            Edition::Public => "发布方",
        }));
        let delayed = crate::bridge::ad_refresh_waiting("ad_refresh_not_due_or_merged", edition);
        assert!(!delayed.recovery.unwrap().contains("Personal"));
        let clock = crate::bridge::ad_refresh_waiting("clock_recovery_required", edition);
        assert!(clock.recovery.unwrap().contains("时间"));
        let local = crate::bridge::ad_refresh_waiting("local_ad_not_published", edition);
        assert!(local.message.contains("本机") && local.message.contains("发布"));
    }
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
fn publisher_input_dispatch_accepts_base64_and_preserves_state_on_invalid_input() {
    use crate::publisher::{security::ManagedVault, test_support::MemoryVault};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let root = SyntheticRoot::new();
    let (_app, window) =
        root.app_with_vault(Arc::new(ManagedVault::synthetic(MemoryVault::default())));
    let image = include_bytes!("../../tests/native-ipc-business/backup-source.png");
    let legacy = invoke(
        &window,
        "import_ad_image",
        json!({"bytes": image.as_slice()}),
    )
    .unwrap();
    let native = invoke(
        &window,
        "import_ad_image",
        json!({"bytes": STANDARD.encode(image)}),
    )
    .unwrap();
    assert_eq!(native, legacy);
    let directory = root.0.join("v2/personal/publisher");
    let before = publisher_tree(&directory);
    for command in [
        "import_ad_image",
        "restore_publisher_backup",
        "publisher_archive_import",
    ] {
        let error = invoke(
            &window,
            command,
            json!({"bytes":"!!!","passphrase":"synthetic-passphrase"}),
        )
        .unwrap_err();
        assert_eq!(error.as_str().unwrap(), "文件数据格式无效");
        assert_eq!(publisher_tree(&directory), before);
    }
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
    let draft = six_campaigns(initial.clone(), &image["asset"]);
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

#[cfg(feature = "personal")]
fn six_campaigns(initial: Value, asset: &Value) -> Value {
    let mut draft = initial;
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
            "image": asset, "button_text": "查看", "target_url": "https://synthetic.example.com/",
            "starts_at": now + j * 3600, "ends_at": now + (j + 1) * 3600, "enabled": true
        })).collect());
    }
    draft
}

#[cfg(feature = "personal")]
mod publisher_backup;
