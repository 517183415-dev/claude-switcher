//! Production handlers with only the secret-store boundary replaced.
use super::*;
use crate::publisher::{backup, security::ManagedVault, test_support::MemoryVault};
use switcher_core::{ad_runtime::local, ads};
use switcher_domain::ads::{AdChannel, AdManifest};

const PASSPHRASE: &str = "synthetic-publisher-backup-password";
const IMAGE: &[u8] = include_bytes!("../../../tests/native-ipc-business/backup-source.png");

fn vault() -> Arc<ManagedVault> {
    Arc::new(ManagedVault::synthetic(MemoryVault::default()))
}

fn generation(root: &SyntheticRoot) -> String {
    root.state_path()
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .into()
}

fn publish(window: &tauri::WebviewWindow<MockRuntime>, channel: &str) {
    let result = invoke(window, "publish_ad", json!({"channel": channel})).unwrap();
    assert_eq!(result["completed"], true);
    assert!(result["message"].as_str().unwrap().contains("公开回读验证"));
}

// Inspect fixed product files independently of the handler's success response.
fn verified(root: &SyntheticRoot, channel: AdChannel) -> AdManifest {
    let directory = local::directory(&root.0);
    let trust: local::LocalTrust =
        serde_json::from_slice(&fs::read(directory.join("trust.json")).unwrap()).unwrap();
    assert_eq!(trust.key_id, "ads-personal-v1");
    let key = ed25519_dalek::VerifyingKey::from_bytes(&trust.public_key).unwrap();
    let name = match channel {
        AdChannel::Preview => "preview",
        AdChannel::Production => "production",
    };
    let wire = fs::read(directory.join(name).join("current.json")).unwrap();
    let (manifest, _) = ads::verify_manifest(
        &wire,
        &key,
        channel,
        "ads-personal-v1",
        switcher_core::checks::unix_now(),
        true,
    )
    .unwrap();
    assert_eq!(
        fs::read(
            directory
                .join(name)
                .join("versions")
                .join(format!("{}.json", manifest.release_id))
        )
        .unwrap(),
        wire
    );
    for campaign in manifest.slots.iter().flat_map(|slot| &slot.campaigns) {
        let path = directory
            .join(name)
            .join("assets")
            .join(format!("{}.png", campaign.image.sha256));
        let bytes = fs::read(&path).unwrap();
        assert_eq!(
            bytes,
            fs::read(
                root.state_path()
                    .parent()
                    .unwrap()
                    .join("assets")
                    .join(&campaign.image.sha256)
            )
            .unwrap()
        );
        assert!(
            !bytes
                .windows(b"SYNTHETIC_PNG_METADATA".len())
                .any(|part| part == b"SYNTHETIC_PNG_METADATA")
        );
        ads::validate_image(&bytes, &campaign.image).unwrap();
        assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
    }
    manifest
}

fn backup_files(root: &SyntheticRoot) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(root.0.join("v2/personal/publisher/backups"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    files
}

#[test]
fn ordinary_dispatch_backup_restores_assets_and_keys_then_republishes_above_current() {
    let root = SyntheticRoot::new();
    let vault = vault();
    let (app, window) = root.app_with_vault(vault.clone());
    let initial = read(&window);
    assert_eq!(
        invoke(&window, "create_ad_signing_key", json!({})).unwrap()["completed"],
        true
    );
    let old_generation = generation(&root);
    let old_secrets = vault.with(|v| v.load(&old_generation)).unwrap();
    // Repeated explicit creation must not rotate a working signing identity.
    assert!(
        invoke(&window, "create_ad_signing_key", json!({}))
            .unwrap_err()
            .as_str()
            .unwrap()
            .contains("signing_key_already_configured")
    );
    let image = invoke(&window, "import_ad_image", json!({"bytes": IMAGE})).unwrap();
    let normalized = fs::read(
        root.state_path()
            .parent()
            .unwrap()
            .join("assets")
            .join(image["asset"]["sha256"].as_str().unwrap()),
    )
    .unwrap();
    assert_ne!(normalized, IMAGE);
    let draft = six_campaigns(initial, &image["asset"]);
    invoke(&window, "ad_draft_save", json!({"manifest":draft})).unwrap();
    publish(&window, "preview");
    assert_eq!(
        serde_json::to_value(verified(&root, AdChannel::Preview)).unwrap()["slots"],
        draft["slots"]
    );
    publish(&window, "production");
    let saved = verified(&root, AdChannel::Production);
    assert_eq!(
        invoke(
            &window,
            "export_publisher_backup",
            json!({"passphrase":PASSPHRASE})
        )
        .unwrap()["completed"],
        true
    );
    let files = backup_files(&root);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].extension().unwrap(), "csbackup");
    assert_eq!(
        fs::metadata(&files[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let encrypted = fs::read(&files[0]).unwrap();
    let plain = backup::open(&encrypted, PASSPHRASE.as_bytes()).unwrap();
    let snapshot: Value = serde_json::from_slice(&plain).unwrap();
    assert_eq!(
        snapshot["state"]["draft"],
        serde_json::to_value(&saved).unwrap()
    );
    assert_eq!(snapshot["assets"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["assets"][0]["bytes"], json!(normalized));
    assert!(
        !encrypted
            .windows(normalized.len())
            .any(|bytes| bytes == normalized)
    );
    invoke(&window, "pause_ad", json!({"slot":null})).unwrap();
    publish(&window, "production");
    let newer = verified(&root, AdChannel::Production);
    assert!(newer.content_revision > saved.content_revision);
    let old_path = root.state_path();
    let old_bytes = fs::read(&old_path).unwrap();
    let public_before = publisher_tree(&local::directory(&root.0));
    let result = invoke(
        &window,
        "restore_publisher_backup",
        json!({"bytes":encrypted,"passphrase":PASSPHRASE}),
    )
    .unwrap();
    assert_eq!(result["completed"], false);
    assert!(result["message"].as_str().unwrap().contains("尚未发布"));
    assert_ne!(generation(&root), old_generation);
    assert_eq!(fs::read(old_path).unwrap(), old_bytes);
    assert_eq!(publisher_tree(&local::directory(&root.0)), public_before);
    let restored = read(&window);
    assert_eq!(restored["slots"], draft["slots"]);
    assert!(restored["content_revision"].as_u64().unwrap() > newer.content_revision);
    let new_secrets = vault.with(|v| v.load(&generation(&root))).unwrap();
    assert!(new_secrets.signing_seed == old_secrets.signing_seed);
    assert!(
        vault
            .with(|v| v.load(&old_generation))
            .unwrap()
            .signing_seed
            == old_secrets.signing_seed
    );
    drop(window);
    drop(app);
    let (_app, window) = root.app_with_vault(vault);
    assert_eq!(read(&window), restored);
    assert_eq!(
        invoke(&window, "publisher_status", json!({})).unwrap()["status"],
        "recovery_required"
    );
    publish(&window, "production");
    let republished = verified(&root, AdChannel::Production);
    assert!(republished.content_revision > newer.content_revision);
    assert_eq!(republished.slots, saved.slots);
    assert_ne!(republished.release_id, saved.release_id);
    assert_eq!(
        invoke(&window, "publisher_status", json!({})).unwrap()["status"],
        "ready_local"
    );
}

#[test]
fn ordinary_dispatch_recovery_confirmation_and_archive_import_preserve_active_data() {
    let root = SyntheticRoot::new();
    let vault = vault();
    let (app, window) = root.app_with_vault(vault.clone());
    invoke(&window, "create_ad_signing_key", json!({})).unwrap();
    invoke(&window, "import_ad_image", json!({"bytes":IMAGE})).unwrap();
    publish(&window, "preview");
    let old_generation = generation(&root);
    let old_path = root.state_path();
    let old_bytes = fs::read(&old_path).unwrap();
    let old_secret = vault.with(|v| v.load(&old_generation)).unwrap();
    let public_before = publisher_tree(&local::directory(&root.0));
    // A canceled preparation keeps its verified backup and releases both locks.
    let canceled = invoke(
        &window,
        "publisher_recovery_prepare",
        json!({"passphrase":PASSPHRASE}),
    )
    .unwrap();
    invoke(&window, "publisher_recovery_cancel", json!({})).unwrap();
    assert_eq!(generation(&root), old_generation);
    assert_eq!(fs::read(&old_path).unwrap(), old_bytes);
    let preview = invoke(
        &window,
        "publisher_recovery_prepare",
        json!({"passphrase":PASSPHRASE}),
    )
    .unwrap();
    assert_ne!(canceled["digest"], preview["digest"]);
    assert_eq!(
        invoke(
            &window,
            "publisher_recovery_confirm",
            json!({"digest":preview["digest"]})
        )
        .unwrap()["completed"],
        true
    );
    assert_ne!(generation(&root), old_generation);
    assert_eq!(fs::read(&old_path).unwrap(), old_bytes);
    assert!(
        vault
            .with(|v| v.load(&old_generation))
            .unwrap()
            .signing_seed
            == old_secret.signing_seed
    );
    assert!(
        vault
            .with(|v| v.load(&generation(&root)))
            .unwrap()
            .signing_seed
            == old_secret.signing_seed
    );
    assert_eq!(publisher_tree(&local::directory(&root.0)), public_before);
    let retained = invoke(&window, "publisher_retained_list", json!({})).unwrap();
    assert!(
        retained
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["generation"] == old_generation && item["state_verified"] == true)
    );
    let before = backup_files(&root);
    invoke(
        &window,
        "publisher_retained_export",
        json!({"generation":old_generation,"passphrase":PASSPHRASE}),
    )
    .unwrap();
    let added: Vec<_> = backup_files(&root)
        .into_iter()
        .filter(|p| !before.contains(p))
        .collect();
    assert_eq!(added.len(), 1);
    let archive = fs::read(&added[0]).unwrap();
    let plain = backup::open(&archive, PASSPHRASE.as_bytes()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&plain).unwrap()["generation"],
        old_generation
    );
    let active_generation = generation(&root);
    let active_draft = read(&window);
    let active_bytes = fs::read(root.state_path()).unwrap();
    let tree_before = publisher_tree(&root.0.join("v2/personal/publisher"));
    assert!(
        invoke(
            &window,
            "publisher_archive_import",
            json!({"bytes":archive,"passphrase":PASSPHRASE})
        )
        .unwrap_err()
        .as_str()
        .unwrap()
        .contains("archive_generation_exists")
    );
    assert_eq!(
        publisher_tree(&root.0.join("v2/personal/publisher")),
        tree_before
    );
    assert_eq!(generation(&root), active_generation);
    assert_eq!(read(&window), active_draft);
    assert_eq!(fs::read(root.state_path()).unwrap(), active_bytes);
    assert_eq!(publisher_tree(&local::directory(&root.0)), public_before);
    let retained = invoke(&window, "publisher_retained_list", json!({})).unwrap();
    assert_eq!(retained.as_array().unwrap().len(), 1);
    // A distinct installation has the same signing identity restored from a
    // portable backup, but neither the retained generation nor its Vault item.
    let before_portable = backup_files(&root);
    invoke(
        &window,
        "export_publisher_backup",
        json!({"passphrase":PASSPHRASE}),
    )
    .unwrap();
    let portable = backup_files(&root)
        .into_iter()
        .find(|p| !before_portable.contains(p))
        .unwrap();
    let destination = SyntheticRoot::new();
    let destination_vault = self::vault();
    let (destination_app, destination_window) =
        destination.app_with_vault(destination_vault.clone());
    invoke(
        &destination_window,
        "restore_publisher_backup",
        json!({"bytes":fs::read(portable).unwrap(),"passphrase":PASSPHRASE}),
    )
    .unwrap();
    let destination_active = destination.state_path();
    let destination_bytes = fs::read(&destination_active).unwrap();
    let destination_generation = generation(&destination);
    assert_eq!(
        invoke(
            &destination_window,
            "publisher_archive_import",
            json!({"bytes":archive,"passphrase":PASSPHRASE})
        )
        .unwrap()["completed"],
        true
    );
    assert_eq!(generation(&destination), destination_generation);
    assert_eq!(fs::read(&destination_active).unwrap(), destination_bytes);
    assert_eq!(
        fs::read(
            destination_active
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(&old_generation)
                .join("state.json")
        )
        .unwrap(),
        old_bytes
    );
    assert!(
        destination_vault
            .with(|v| v.load(&old_generation))
            .unwrap()
            .signing_seed
            == old_secret.signing_seed
    );
    assert!(!local::directory(&destination.0).exists());
    let imported = invoke(&destination_window, "publisher_retained_list", json!({})).unwrap();
    assert_eq!(imported.as_array().unwrap().len(), 1);
    assert_eq!(imported[0]["generation"], old_generation);
    assert_eq!(imported[0]["state_verified"], true);
    drop(destination_window);
    drop(destination_app);
    let (_destination_app, destination_window) = destination.app_with_vault(destination_vault);
    assert_eq!(
        fs::read(destination.state_path()).unwrap(),
        destination_bytes
    );
    assert_eq!(
        invoke(&destination_window, "publisher_retained_list", json!({})).unwrap(),
        imported
    );
    drop(window);
    drop(app);
    let (_app, window) = root.app_with_vault(vault);
    assert_eq!(read(&window), active_draft);
    assert_eq!(
        invoke(&window, "publisher_retained_list", json!({})).unwrap(),
        retained
    );
}
