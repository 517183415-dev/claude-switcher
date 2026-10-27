//! Feature-only publication fixture. The vault exists only in process memory.
use super::Fixture;
use crate::{
    bridge::DesktopState,
    publisher::{security::Secrets, test_support::MemoryVault},
    publisher_commands,
};
use std::sync::{Arc, Mutex};
use switcher_domain::{ads::AdChannel, desktop::OperationOutcome};

#[derive(Default)]
pub(super) struct PublishFixture(Mutex<Option<MemoryVault>>);

#[tauri::command]
pub(super) fn ipc_business_publish_init(
    fixture: tauri::State<'_, Fixture>,
    vault: tauri::State<'_, PublishFixture>,
    state: tauri::State<'_, Arc<DesktopState>>,
) -> Result<(), String> {
    if !matches!(fixture.publish_phase, Some("write" | "resume")) {
        return Err("fixture_mode".into());
    }
    let mut vault = vault.0.lock().map_err(|_| "fixture_lock")?;
    if vault.is_some() {
        return Err("fixture_already_initialized".into());
    }
    let mut synthetic = MemoryVault::default();
    publisher_commands::with_publisher(&state, |publisher| {
        publisher.credentials_save(
            &mut synthetic,
            &Secrets {
                signing_seed: vec![0x41; 32],
                preview_credential: b"synthetic-preview-only".to_vec(),
                production_credential: b"synthetic-production-only".to_vec(),
            },
        )
    })?;
    *vault = Some(synthetic);
    Ok(())
}

#[tauri::command]
pub(super) fn ipc_business_publish(
    fixture: tauri::State<'_, Fixture>,
    vault: tauri::State<'_, PublishFixture>,
    state: tauri::State<'_, Arc<DesktopState>>,
    channel: AdChannel,
) -> Result<OperationOutcome, String> {
    if !matches!(fixture.publish_phase, Some("write" | "resume")) {
        return Err("fixture_mode".into());
    }
    let vault = vault.0.lock().map_err(|_| "fixture_lock")?;
    let vault = vault.as_ref().ok_or("fixture_not_initialized")?;
    publisher_commands::with_publisher(&state, |publisher| {
        publisher_commands::publish_local(
            publisher,
            &state.root,
            channel,
            switcher_core::checks::unix_now(),
            vault,
        )
    })
}
