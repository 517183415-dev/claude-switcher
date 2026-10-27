use tauri::Manager;

unsafe extern "C" {
    fn switcher_native_attach(webview: *mut std::ffi::c_void, public_edition: i32);
}

pub(crate) fn use_bridge_page(context: &mut tauri::Context<tauri::Wry>) {
    if let Some(window) = context
        .config_mut()
        .app
        .windows
        .iter_mut()
        .find(|window| window.label == "main")
    {
        window.url = tauri::WebviewUrl::App("native-bridge.html".into());
    }
}

pub(crate) fn attach(
    app: &mut tauri::App,
    public_edition: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or("native UI main window missing")?;
    window.with_webview(move |webview| {
        // Tauri retains the WKWebView. Swift acquires a reference before scheduling UI work.
        unsafe { switcher_native_attach(webview.inner(), i32::from(public_edition)) }
    })?;
    Ok(())
}
