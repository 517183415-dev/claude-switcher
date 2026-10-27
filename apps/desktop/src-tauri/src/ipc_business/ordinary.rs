//! Real WebView and product registry; only this test feature substitutes secret storage.
use crate::publisher::{security::ManagedVault, test_support::MemoryVault};
use std::sync::Arc;

pub(super) fn run() {
    assert!(
        std::env::var("IPC_ORDINARY_PUBLISHER_OPT_IN").as_deref()
            == Ok("synthetic-memory-vault-only")
    );
    let state = crate::bridge::DesktopState::isolated_activity_fixture(
        super::synthetic_root(),
        switcher_domain::Edition::Personal,
    );
    eprintln!(
        "ORDINARY_PUBLISHER_FIXTURE synthetic-memory-vault-only; shared product registry; test frontend"
    );
    tauri::Builder::default()
        .manage(Arc::new(state))
        .manage(Arc::new(ManagedVault::synthetic(MemoryVault::default())))
        .manage(Arc::new(
            crate::publisher_recovery_commands::RecoverySessions::default(),
        ))
        .invoke_handler(crate::ipc::handler())
        .run(tauri::generate_context!())
        .expect("isolated ordinary publisher WebView runtime");
}
