//! Real command decoding, ordinary ACL and publisher handlers on Tauri MockRuntime.
//! This does not exercise WKWebView, native Keychain or external publication.
use crate::{bridge::DesktopState, publisher_commands as commands};
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
            switcher_domain::Edition::Personal,
        );
        let mut context = mock_context(noop_assets());
        *context.runtime_authority_mut() = crate::ipc_policy_tests::ordinary_policy().0;
        let app = mock_builder()
            .manage(Arc::new(state))
            .invoke_handler(tauri::generate_handler![
                commands::ad_draft_read,
                commands::ad_draft_save,
                commands::pause_ad,
                commands::import_ad_image,
                commands::ad_image_preview,
                commands::publisher_status
            ])
            .build(context)
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        (app, window)
    }
    fn state_path(&self) -> PathBuf {
        let root = self.0.join("v2/personal/publisher");
        let active: Value =
            serde_json::from_slice(&fs::read(root.join("active.json")).unwrap()).unwrap();
        root.join(active["generation"].as_str().unwrap())
            .join("state.json")
    }
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
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost/".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}
fn read(window: &tauri::WebviewWindow<MockRuntime>) -> Value {
    invoke(window, "ad_draft_read", json!({})).unwrap()
}

#[test]
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
