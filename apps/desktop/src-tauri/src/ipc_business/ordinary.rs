//! Real WebView and product registry; only this test feature substitutes secret storage.
#[cfg(feature = "personal")]
use crate::publisher::{security::ManagedVault, test_support::MemoryVault};
use std::sync::Arc;

pub(super) fn run() {
    assert!(
        std::env::var("IPC_ORDINARY_PUBLISHER_OPT_IN").as_deref()
            == Ok("synthetic-memory-vault-only")
    );
    let state = crate::bridge::DesktopState::isolated_activity_fixture(
        super::synthetic_root(),
        if cfg!(feature = "personal") {
            switcher_domain::Edition::Personal
        } else {
            switcher_domain::Edition::Public
        },
    );
    eprintln!(
        "ORDINARY_PUBLISHER_FIXTURE synthetic-memory-vault-only; shared product registry; test frontend"
    );
    let builder = tauri::Builder::default().manage(Arc::new(state));
    #[cfg(feature = "personal")]
    let builder = builder
        .manage(Arc::new(ManagedVault::synthetic(MemoryVault::default())))
        .manage(Arc::new(
            crate::publisher_recovery_commands::RecoverySessions::default(),
        ));
    let builder = builder.invoke_handler(crate::ipc::handler());
    #[cfg(all(feature = "native-ui", target_os = "macos"))]
    let builder = builder.setup(|app| crate::native_host::attach(app, cfg!(feature = "public")));
    #[allow(unused_mut)]
    let mut context = tauri::generate_context!();
    #[cfg(all(feature = "native-ui", target_os = "macos"))]
    crate::native_host::use_bridge_page(&mut context);
    builder
        .run(context)
        .expect("isolated ordinary publisher WebView runtime");
}
