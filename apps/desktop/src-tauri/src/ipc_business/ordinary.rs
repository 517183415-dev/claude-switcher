//! Real WebView and product registry; only this test feature substitutes secret storage.
#[cfg(feature = "personal")]
use crate::publisher::{security::ManagedVault, test_support::MemoryVault};
use std::sync::Arc;

#[cfg(feature = "personal")]
fn prepare_valid_large_backup() {
    if std::env::var("NATIVE_UI_FIXTURE_VALID_LARGE_BACKUP").as_deref() != Ok("1") {
        return;
    }
    use crate::publisher::{Publisher, backup};
    let tmp = std::env::temp_dir().canonicalize().expect("fixture TMPDIR");
    let mut publisher = Publisher::open(
        &tmp.join("large-backup-source"),
        "ads-personal-v1",
        None,
        switcher_core::checks::unix_now(),
    )
    .expect("isolated publisher");
    let mut vault = MemoryVault::default();
    publisher
        .generate_signing_key(&mut vault, Vec::new(), Vec::new())
        .expect("synthetic signing key");
    let password = b"synthetic-passphrase";
    let backup = publisher.export_backup(password, &vault).expect("backup");
    let mut plaintext = backup::open(&backup, password).expect("backup plaintext");
    let envelope_size = backup.len() - plaintext.len();
    // JSON whitespace creates an authenticated boundary-sized container without
    // claiming to represent a 64 MiB corpus of user assets.
    plaintext.resize(backup::LIMIT - envelope_size, b' ');
    let large = backup::seal(&plaintext, password).expect("large backup");
    assert_eq!(large.len(), backup::LIMIT);
    switcher_core::storage::atomic_write(&tmp.join("valid-large.csbackup"), &large)
        .expect("isolated backup file");
}

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
    #[cfg(feature = "personal")]
    prepare_valid_large_backup();
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
