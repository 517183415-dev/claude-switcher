fn main() {
    let app = tauri_build::AppManifest::new().commands(&["probe", "finish", "forbidden_probe"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app)).expect("Tauri manifest");
}
