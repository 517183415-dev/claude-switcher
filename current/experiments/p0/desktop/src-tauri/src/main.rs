mod contract;
use contract::ProbeResult;

#[tauri::command]
fn probe() -> ProbeResult {
    ProbeResult { edition: "personal".into(), slots: ["residential_broadband".into(), "account_service".into(), "kyc_service".into()] }
}
#[tauri::command]
fn forbidden_probe() { panic!("P0 ACL failure: forbidden command reached Rust"); }
#[tauri::command]
fn finish(app: tauri::AppHandle, ok: bool, denied: bool, rendered: bool) {
    println!("P0_WEBVIEW_RESULT {}", serde_json::json!({"ok":ok,"custom_command_denied":denied,"three_slots_rendered":rendered,"ui_to_rust_ipc":true}));
    app.exit(if ok {0} else {1});
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![probe, finish, forbidden_probe])
        .run(tauri::generate_context!()).expect("P0 Tauri runtime");
}
