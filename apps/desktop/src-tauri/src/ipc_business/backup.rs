//! Synthetic encrypted bytes cross IPC; only public metadata and invariants leave this fixture.
use crate::publisher::{
    self, Publisher,
    security::{Secrets, Vault},
    test_support::MemoryVault,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use switcher_core::storage;

const NOW: u64 = 1_700_000_000;
const KEY_ID: &str = "ipc-synthetic-backup";
pub(super) struct BackupFixture(Mutex<Option<Harness>>);
struct Harness {
    root: PathBuf,
    vault: MemoryVault,
    source_vault: MemoryVault,
    source_generation: String,
    source_active: Vec<u8>,
    source_state: Vec<u8>,
    index: usize,
}
impl BackupFixture {
    pub(super) fn new() -> Self {
        Self(Mutex::new(None))
    }
}
fn open(path: &Path) -> Result<Publisher, String> {
    Publisher::open(path, KEY_ID, None, NOW).map_err(|_| "fixture_open".into())
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "fixture_read")?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "fixture_read")?;
    if bytes.len() > 65536 {
        return Err("fixture_read_bound".into());
    }
    Ok(bytes)
}
fn generation(root: &Path) -> Result<String, String> {
    let value: Value =
        serde_json::from_slice(&read(&root.join("active.json"))?).map_err(|_| "fixture_active")?;
    let id = value["generation"].as_str().ok_or("fixture_generation")?;
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("fixture_generation".into());
    }
    Ok(id.into())
}
fn credentials(publisher: &mut Publisher, marker: u8) -> Result<MemoryVault, String> {
    let mut vault = MemoryVault::default();
    publisher
        .credentials_save(
            &mut vault,
            &Secrets {
                signing_seed: vec![0x41; 32],
                preview_credential: vec![marker; 24],
                production_credential: vec![marker + 1; 24],
            },
        )
        .map_err(|_| "fixture_credentials")?;
    Ok(vault)
}
fn prepare_directory(edition: &Path) -> Result<PathBuf, String> {
    storage::create_private_directory(edition.parent().ok_or("fixture_parent")?)
        .map_err(|_| "fixture_directory")?;
    storage::create_private_directory(edition).map_err(|_| "fixture_directory")?;
    let root = edition.join("backup-fixture");
    storage::create_private_directory(&root).map_err(|_| "fixture_directory")?;
    Ok(root)
}

#[tauri::command]
pub(super) fn ipc_business_backup_export(
    fixture: tauri::State<'_, BackupFixture>,
    state: tauri::State<'_, std::sync::Arc<crate::bridge::DesktopState>>,
    passphrase: String,
) -> Result<Vec<u8>, String> {
    let passphrase = zeroize::Zeroizing::new(passphrase);
    let mut locked = fixture.0.lock().map_err(|_| "fixture_lock")?;
    if locked.is_some() {
        return Err("fixture_already_initialized".into());
    }
    let root = prepare_directory(&state.directory())?;
    let source_root = root.join("source");
    let mut source = open(&source_root)?;
    let source_vault = credentials(&mut source, 0x62)?;
    let mut draft = source.draft_read();
    for (slot, enabled) in draft.slots.iter_mut().zip([true, false, true]) {
        slot.enabled = enabled;
    }
    source.draft_save(draft, NOW).map_err(|_| "fixture_draft")?;
    let bytes = source
        .export_backup(passphrase.as_bytes(), &source_vault)
        .map_err(|_| "fixture_export")?;
    let source_generation = generation(&source_root)?;
    let source_active = read(&source_root.join("active.json"))?;
    let source_state = read(&source_root.join(&source_generation).join("state.json"))?;
    let mut target = open(&root.join("target"))?;
    let vault = credentials(&mut target, 0x72)?;
    target.pause(None, NOW).map_err(|_| "fixture_pause")?;
    *locked = Some(Harness {
        root,
        vault,
        source_vault,
        source_generation,
        source_active,
        source_state,
        index: 0,
    });
    Ok(bytes)
}
#[tauri::command]
pub(super) fn ipc_business_backup_restore(
    fixture: tauri::State<'_, BackupFixture>,
    case: String,
    bytes: Vec<u8>,
    passphrase: String,
) -> Result<Value, String> {
    let mut guard = fixture.0.lock().map_err(|_| "fixture_lock")?;
    let harness = guard.as_mut().ok_or("fixture_not_initialized")?;
    let expected = [
        "wrong_password",
        "tampered",
        "truncated",
        "vault_failure",
        "success",
    ];
    if expected.get(harness.index).copied() != Some(case.as_str()) {
        return Err("fixture_case_order".into());
    }
    let root = harness.root.join("target");
    let source_root = harness.root.join("source");
    let mut target = open(&root)?;
    let old_generation = generation(&root)?;
    let old_active = read(&root.join("active.json"))?;
    let old_state = read(&root.join(&old_generation).join("state.json"))?;
    let old_trust = read(&root.join("trust.json"))?;
    let old_vault = MemoryVault {
        items: harness.vault.items.clone(),
        fail: false,
    };
    harness.vault.fail = case == "vault_failure";
    let mut entered = 0;
    let mut error_code = None;
    let result = crate::publisher_commands::restore_publisher_backup_body(
        &bytes,
        passphrase,
        |bytes, password| {
            entered += 1;
            target
                .restore_backup(bytes, password, &mut harness.vault)
                .map_err(|error| {
                    error_code = Some(match error {
                        publisher::Error::Invalid("backup_password_or_integrity") => {
                            "password_or_integrity"
                        }
                        publisher::Error::Invalid("backup_header") => "header",
                        publisher::Error::SecretUnavailable => "vault_unavailable",
                        _ => "unexpected_restore_error",
                    });
                    "synthetic_restore_rejected".into()
                })
        },
    );
    harness.vault.fail = false;
    drop(target);
    let reopened = open(&root)?;
    let new_generation = generation(&root)?;
    let source_unchanged = read(&source_root.join("active.json"))? == harness.source_active
        && read(
            &source_root
                .join(&harness.source_generation)
                .join("state.json"),
        )? == harness.source_state;
    let source_secrets = harness
        .source_vault
        .load(&harness.source_generation)
        .map_err(|_| "source_vault_load")?;
    let reopened_secrets = harness
        .vault
        .load(&new_generation)
        .map_err(|_| "reopened_vault_load")?;
    let same_secrets = |left: &Secrets, right: &Secrets| {
        left.signing_seed == right.signing_seed
            && left.preview_credential == right.preview_credential
            && left.production_credential == right.production_credential
    };
    let old_secrets = old_vault
        .load(&old_generation)
        .map_err(|_| "old_vault_load")?;
    let expected_secrets = if case == "success" {
        &source_secrets
    } else {
        &old_secrets
    };
    let receipt = json!({
        "case": case, "entered": entered, "accepted": result.is_ok(), "error_code": error_code,
        "old_generation": old_generation, "new_generation": new_generation,
        "active_unchanged": read(&root.join("active.json"))? == old_active,
        "old_generation_preserved": read(&root.join(&old_generation).join("state.json"))? == old_state,
        "trust_unchanged": read(&root.join("trust.json"))? == old_trust,
        "old_vault_preserved": old_vault.items.iter().all(|(key, value)| harness.vault.items.get(key) == Some(value)),
        "vault_items": harness.vault.items.len(), "reopened_credentials_match": same_secrets(&reopened_secrets, expected_secrets),
        "reopened_slots": reopened.draft_read().slots.iter().map(|slot| slot.enabled).collect::<Vec<_>>(),
        "recovery_required": reopened.status().recovery_required, "source_unchanged": source_unchanged,
    });
    harness.index += 1;
    println!("BUSINESS_IPC_BACKUP {receipt}");
    // No byte buffers, passphrases, credentials, public keys or state snapshots are logged.
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn backup_fixture_initializes_missing_edition_parents_privately() {
        let root = std::env::temp_dir().join(format!(
            "backup-fixture-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        storage::create_private_directory(&root).unwrap();
        let result = prepare_directory(&root.join("v2/personal"));
        // Always remove the synthetic root, including on the regression's RED result.
        let checks = result.as_ref().map(|path| {
            [root.join("v2"), root.join("v2/personal"), path.clone()]
                .iter()
                .all(|path| {
                    fs::symlink_metadata(path).is_ok_and(|metadata| {
                        metadata.is_dir() && metadata.permissions().mode() & 0o777 == 0o700
                    })
                })
        });
        fs::remove_dir_all(&root).unwrap();
        assert!(
            matches!(checks, Ok(true)),
            "missing parent initialization must produce private real directories"
        );
    }
}
