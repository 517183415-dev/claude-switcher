use super::super::{
    protocol::{Object, StoreError},
    test_support::MemoryVault,
};
use super::*;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
use switcher_core::storage;

#[path = "maintenance_tests.rs"]
mod maintenance_tests;
#[path = "r2_integration_tests.rs"]
mod r2_integration_tests;

struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Root, Publisher, MemoryVault) {
    let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "switcher-remote-{}",
        super::super::random_id().unwrap()
    ));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    let root = Root(path);
    storage::create_private_directory(&root.0.join("v2")).unwrap();
    let mut publisher =
        Publisher::open(&root.0.join("publisher"), "ads-personal-v1", None, 1000).unwrap();
    let mut vault = MemoryVault::default();
    publisher
        .credentials_save(
            &mut vault,
            &Secrets {
                signing_seed: vec![42; 32],
                preview_credential: vec![],
                production_credential: vec![],
            },
        )
        .unwrap();
    (root, publisher, vault)
}
fn config(channel: AdChannel) -> R2Config {
    let (bucket, host) = match channel {
        AdChannel::Preview => ("ads-preview", "preview.example.com"),
        AdChannel::Production => ("ads-production", "ads.example.com"),
    };
    R2Config {
        account_id: "abcdef0123456789abcdef0123456789".into(),
        bucket: bucket.into(),
        public_origin: format!("https://{host}"),
        channel,
    }
}
fn configure(
    p: &mut Publisher,
    v: &mut dyn Vault,
    channel: AdChannel,
    key: &str,
) -> Result<RemoteSettings> {
    p.configure_r2(
        channel,
        Some(config(channel)),
        Zeroizing::new(key.into()),
        Zeroizing::new(format!("secret-{key}")),
        v,
    )
}

// Real on-disk object handling with a synthetic R2 identity. The separately
// tested R2 SDK transport is not contacted by these connection tests.
struct TestRemote {
    inner: LocalObjectStore,
    target: String,
    unknown: bool,
    lock_path: PathBuf,
    lock_id: String,
}
impl ObjectStore for TestRemote {
    fn lock_identity(&self) -> Option<String> {
        Some(self.lock_id.clone())
    }
    fn read_lock(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        match storage::read_private_bytes(&self.lock_path, super::super::remote_lock::WIRE_LIMIT) {
            Ok(bytes) => Ok(Some(Object {
                etag: switcher_core::signed::digest(&bytes),
                bytes,
            })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(StoreError::Unavailable),
        }
    }
    fn commit_lock(
        &mut self,
        expected: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        if self.read_lock()?.as_ref().map(|o| o.etag.as_str()) != expected {
            return Err(StoreError::Conflict);
        }
        storage::atomic_write(&self.lock_path, bytes).map_err(|_| StoreError::Unavailable)
    }

    fn target_identity(&self) -> Option<String> {
        Some(self.target.clone())
    }
    fn channel(&self) -> AdChannel {
        self.inner.channel()
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        self.inner.current()
    }
    fn public_read(
        &mut self,
        key: &str,
        limit: usize,
    ) -> std::result::Result<Option<Object>, StoreError> {
        self.inner.public_read(key, limit)
    }
    fn put_immutable(&mut self, key: &str, bytes: &[u8]) -> std::result::Result<(), StoreError> {
        self.inner.put_immutable(key, bytes)
    }
    fn commit_current(
        &mut self,
        etag: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        if self.unknown {
            Err(StoreError::OutcomeUnknown)
        } else {
            self.inner.commit_current(etag, bytes)
        }
    }
}
fn remote(root: &Path, c: R2Config, unknown: bool) -> Result<Box<dyn ObjectStore>> {
    let base = root.join(&c.bucket);
    storage::create_private_directory(&base)?;
    storage::create_private_directory(&base.join("v2"))?;
    let trust = switcher_core::ad_runtime::local::LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: ed25519_dalek::SigningKey::from_bytes(&[42; 32])
            .verifying_key()
            .to_bytes(),
    };
    Ok(Box::new(TestRemote {
        inner: LocalObjectStore::open(&base, c.channel, &trust)?,
        target: c.identity()?,
        lock_id: c.lock_identity()?,
        lock_path: base.join(format!(
            "lock-{}.json",
            super::super::protocol::prefix(c.channel)
        )),
        unknown,
    }))
}

#[test]
fn missing_saved_credentials_are_not_reported_as_local_sources() {
    let (root, mut publisher, mut vault) = fixture();
    let fresh = Publisher::open(&root.0.join("fresh"), "ads-personal-v1", None, 1000).unwrap();
    let initial = fresh.remote_settings(&MemoryVault::default()).unwrap();
    assert!(initial.preview.is_none() && initial.production.is_none());

    configure(
        &mut publisher,
        &mut vault,
        AdChannel::Preview,
        "preview-key",
    )
    .unwrap();
    publisher
        .publish_configured_with(&root.0, AdChannel::Preview, 1001, &vault, |config, _, _| {
            remote(&root.0, config, false)
        })
        .unwrap();
    let saved = vault.items.remove(&publisher.generation).unwrap();
    assert!(matches!(
        publisher.remote_settings(&vault),
        Err(Error::SecretUnavailable)
    ));
    vault.items.insert(publisher.generation.clone(), saved);
    assert!(publisher.remote_settings(&vault).unwrap().preview.is_some());

    drop(publisher);
    fs::remove_file(root.0.join("publisher/trust.json")).unwrap();
    let reopened =
        Publisher::open(&root.0.join("publisher"), "ads-personal-v1", None, 1000).unwrap();
    assert!(reopened.configured_public_key().is_none());
    assert!(reopened.active_target().is_some());
    assert!(matches!(
        reopened.remote_settings(&MemoryVault::default()),
        Err(Error::SecretUnavailable)
    ));
}

#[test]
fn settings_keep_credentials_out_of_responses_and_disk_and_survive_restart() {
    let (root, mut p, mut v) = fixture();
    assert!(p.remote_settings(&v).unwrap().preview.is_none());
    let view = configure(&mut p, &mut v, AdChannel::Preview, "private-preview-key").unwrap();
    let response = serde_json::to_string(&view).unwrap();
    assert!(response.contains("ads-preview"));
    assert!(!response.contains("private-preview-key") && !response.contains("secret-"));
    fn no_credentials(path: &Path) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                no_credentials(&entry.path());
            } else {
                assert!(
                    !String::from_utf8_lossy(&fs::read(entry.path()).unwrap())
                        .contains("private-preview-key")
                );
            }
        }
    }
    no_credentials(&root.0);
    drop(p);
    let p = Publisher::open(&root.0.join("publisher"), "ads-personal-v1", None, 1001).unwrap();
    assert_eq!(
        p.remote_settings(&v).unwrap().preview.unwrap().bucket,
        "ads-preview"
    );
}

#[test]
fn invalid_configurations_and_shared_channel_credentials_preserve_vault() {
    let (_root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    for variant in 0..7 {
        let mut c = config(AdChannel::Production);
        let mut key = "production-key";
        match variant {
            0 => c.public_origin = "https://private.r2.dev".into(),
            1 => c.public_origin = "http://ads.example.com".into(),
            2 => c.channel = AdChannel::Preview,
            3 => c.bucket = "ads-preview".into(),
            4 => key = "preview-key",
            5 => key = "key\ncredential",
            _ => c.account_id = "untrusted-endpoint".into(),
        }
        let before = v.items.clone();
        let error = p
            .configure_r2(
                AdChannel::Production,
                Some(c),
                Zeroizing::new(key.into()),
                Zeroizing::new("private-secret-value".into()),
                &mut v,
            )
            .err()
            .unwrap();
        assert_eq!(v.items, before);
        assert!(!error.to_string().contains("private-secret-value"));
    }
}

#[test]
fn vault_write_failure_preserves_previous_configuration() {
    let (_root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    let before = v.items.clone();
    v.fail = true;
    assert!(configure(&mut p, &mut v, AdChannel::Preview, "rotated-key").is_err());
    assert_eq!(v.items, before);
}

#[test]
fn corrupt_credentials_never_fall_back_to_a_local_publication() {
    let (root, mut p, mut v) = fixture();
    let mut secrets = v.load(&p.generation).unwrap();
    secrets.preview_credential = b"invalid-private-credential".to_vec();
    p.credentials_save(&mut v, &secrets).unwrap();
    let before = super::super::encode(&p.state, super::super::STATE_LIMIT).unwrap();
    assert!(
        p.publish_configured_with(&root.0, AdChannel::Preview, 1001, &v, |_, _, _| panic!(
            "invalid config must not open a store"
        ))
        .is_err()
    );
    assert_eq!(
        super::super::encode(&p.state, super::super::STATE_LIMIT).unwrap(),
        before
    );
}

#[test]
fn configured_channels_route_and_reopen_with_separate_target_histories() {
    let (root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    configure(&mut p, &mut v, AdChannel::Production, "production-key").unwrap();
    for (channel, expected_key) in [
        (AdChannel::Preview, "preview-key"),
        (AdChannel::Production, "production-key"),
    ] {
        p.publish_configured_with(&root.0, channel, 1002, &v, |c, key, secret| {
            assert_eq!(key, expected_key);
            assert_eq!(secret, format!("secret-{expected_key}"));
            remote(&root.0, c, false)
        })
        .unwrap();
        assert_eq!(p.state.history.len(), 1);
    }
    drop(p);
    let mut p = Publisher::open(&root.0.join("publisher"), "ads-personal-v1", None, 1003).unwrap();
    p.publish_configured_with(&root.0, AdChannel::Preview, 1003, &v, |c, _, _| {
        remote(&root.0, c, false)
    })
    .unwrap();
    assert_eq!(
        p.active_target(),
        Some(config(AdChannel::Preview).identity().unwrap().as_str())
    );
    assert!(
        p.state
            .history
            .iter()
            .all(|h| h.manifest.channel == AdChannel::Preview)
    );
    p.configure_r2(
        AdChannel::Preview,
        None,
        Zeroizing::new(String::new()),
        Zeroizing::new(String::new()),
        &mut v,
    )
    .unwrap();
    p.publish_configured_with(&root.0, AdChannel::Preview, 1004, &v, |_, _, _| {
        panic!("local channel must not open R2")
    })
    .unwrap();
    assert!(p.active_target().unwrap().starts_with("local:"));
    assert_eq!(p.state.other_targets.len(), 2);
}

#[test]
fn pending_target_cannot_be_removed_or_changed_but_credentials_can_rotate() {
    let (root, mut p, mut v) = fixture();
    let mut alternative = config(AdChannel::Preview);
    alternative.bucket = "alternative-preview".into();
    let alternative_id = alternative.identity().unwrap();
    p.configure_r2(
        AdChannel::Preview,
        Some(alternative),
        Zeroizing::new("alternative-key".into()),
        Zeroizing::new("alternative-secret".into()),
        &mut v,
    )
    .unwrap();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    assert!(
        p.publish_configured_with(&root.0, AdChannel::Preview, 1001, &v, |c, _, _| remote(
            &root.0, c, true
        ))
        .is_err()
    );
    let pending = p
        .state
        .pending
        .as_ref()
        .unwrap()
        .manifest
        .release_id
        .clone();
    // Move it to an inactive ledger; changing its credentials must remain gated.
    p.publish_configured_with(&root.0, AdChannel::Production, 1002, &v, |_, _, _| {
        panic!("unconfigured production is local")
    })
    .unwrap();
    let before = v.items.clone();
    assert!(matches!(
        p.select_retained_r2(AdChannel::Preview, &alternative_id, &mut v),
        Err(Error::PendingOperation)
    ));
    assert!(matches!(
        p.configure_r2(
            AdChannel::Preview,
            None,
            Zeroizing::new(String::new()),
            Zeroizing::new(String::new()),
            &mut v
        ),
        Err(Error::PendingOperation)
    ));
    let mut changed = config(AdChannel::Preview);
    changed.bucket = "different-preview".into();
    assert!(
        p.configure_r2(
            AdChannel::Preview,
            Some(changed),
            Zeroizing::new("new-key".into()),
            Zeroizing::new("new-secret".into()),
            &mut v
        )
        .is_err()
    );
    assert_eq!(v.items, before);
    configure(&mut p, &mut v, AdChannel::Preview, "rotated-key").unwrap();
    p.publish_configured_with(&root.0, AdChannel::Preview, 1003, &v, |c, key, _| {
        assert_eq!(key, "rotated-key");
        remote(&root.0, c, false)
    })
    .unwrap();
    assert_eq!(p.state.history[0].manifest.release_id, pending);
    assert!(p.state.pending.is_none());
}

#[test]
fn changing_remote_target_keeps_previous_access_material() {
    let (_root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "old-private-key").unwrap();
    let mut next = config(AdChannel::Preview);
    next.bucket = "replacement-preview".into();
    p.configure_r2(
        AdChannel::Preview,
        Some(next),
        Zeroizing::new("next-private-key".into()),
        Zeroizing::new("next-private-secret".into()),
        &mut v,
    )
    .unwrap();
    let secrets = v.load(&p.generation).unwrap();
    let bytes = String::from_utf8_lossy(&secrets.preview_credential);
    assert!(
        bytes.contains("ads-preview"),
        "old target configuration was discarded"
    );
    assert!(
        bytes.contains("old-private-key"),
        "old target credential was discarded"
    );
    assert_eq!(
        p.remote_settings(&v).unwrap().preview.unwrap().bucket,
        "replacement-preview"
    );
}

#[test]
fn encrypted_backup_restores_remote_settings_into_the_new_generation() {
    let (_source, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    let bytes = p.export_backup(b"synthetic-backup-password", &v).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("preview-key"));
    let (_dest, mut restored, mut dest_vault) = fixture();
    restored
        .restore_backup(&bytes, b"synthetic-backup-password", &mut dest_vault)
        .unwrap();
    assert_eq!(
        restored
            .remote_settings(&dest_vault)
            .unwrap()
            .preview
            .unwrap()
            .bucket,
        "ads-preview"
    );
}

#[test]
fn retained_target_can_be_selected_after_restart_and_local_selection() {
    let (root, mut p, mut v) = fixture();
    let old = config(AdChannel::Preview).identity().unwrap();
    configure(&mut p, &mut v, AdChannel::Preview, "old-private-key").unwrap();
    p.publish_configured_with(&root.0, AdChannel::Preview, 1001, &v, |c, _, _| {
        remote(&root.0, c, false)
    })
    .unwrap();
    let old_history = p.state.history.clone();
    let mut next = config(AdChannel::Preview);
    next.bucket = "next-preview".into();
    p.configure_r2(
        AdChannel::Preview,
        Some(next),
        Zeroizing::new("next-key".into()),
        Zeroizing::new("next-secret".into()),
        &mut v,
    )
    .unwrap();
    p.configure_r2(
        AdChannel::Preview,
        None,
        Zeroizing::new(String::new()),
        Zeroizing::new(String::new()),
        &mut v,
    )
    .unwrap();
    drop(p);
    let mut p = Publisher::open(&root.0.join("publisher"), "ads-personal-v1", None, 1002).unwrap();
    let view = p.remote_settings(&v).unwrap();
    assert!(view.preview.is_none());
    assert_eq!(view.targets.len(), 2);
    assert!(view.targets.iter().all(|target| !target.selected));
    assert!(view.unavailable_targets.is_empty());
    let view = p
        .select_retained_r2(AdChannel::Preview, &old, &mut v)
        .unwrap();
    assert_eq!(view.preview.unwrap().bucket, "ads-preview");
    p.publish_configured_with(&root.0, AdChannel::Preview, 1003, &v, |c, key, secret| {
        assert_eq!(key, "old-private-key");
        assert_eq!(secret, "secret-old-private-key");
        remote(&root.0, c, false)
    })
    .unwrap();
    assert_eq!(p.state.history[0].manifest, old_history[0].manifest);
    assert_eq!(p.active_target(), Some(old.as_str()));
}

#[test]
fn retained_target_selection_rejects_wrong_channel_unknown_identity_and_vault_failure() {
    let (_root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    let target = config(AdChannel::Preview).identity().unwrap();
    for (channel, target) in [
        (AdChannel::Production, target.as_str()),
        (AdChannel::Preview, "r2:unknown"),
        (AdChannel::Preview, "https://untrusted.example.com"),
    ] {
        let before = v.items.clone();
        assert!(p.select_retained_r2(channel, target, &mut v).is_err());
        assert_eq!(v.items, before);
    }
    p.configure_r2(
        AdChannel::Preview,
        None,
        Zeroizing::new(String::new()),
        Zeroizing::new(String::new()),
        &mut v,
    )
    .unwrap();
    let before = v.items.clone();
    v.fail = true;
    assert!(
        p.select_retained_r2(AdChannel::Preview, &target, &mut v)
            .is_err()
    );
    assert_eq!(v.items, before);
}

#[test]
fn retained_target_registry_migrates_legacy_and_rejects_malformed_records_atomically() {
    let (root, mut p, mut v) = fixture();
    let legacy = serde_json::json!({"schema":1,"config":config(AdChannel::Preview),"access_key":"legacy-private-key","secret_key":"legacy-private-secret"});
    let mut secrets = v.load(&p.generation).unwrap();
    secrets.preview_credential = serde_json::to_vec(&legacy).unwrap();
    p.credentials_save(&mut v, &secrets).unwrap();
    assert_eq!(p.remote_settings(&v).unwrap().targets.len(), 1);
    p.configure_r2(
        AdChannel::Preview,
        None,
        Zeroizing::new(String::new()),
        Zeroizing::new(String::new()),
        &mut v,
    )
    .unwrap();
    let valid = serde_json::from_slice::<serde_json::Value>(
        &v.load(&p.generation).unwrap().preview_credential,
    )
    .unwrap();
    let id = config(AdChannel::Preview).identity().unwrap();
    for case in 0..8 {
        let mut invalid = valid.clone();
        match case {
            0 => invalid["schema"] = 99.into(),
            1 => invalid["selected"] = "r2:unknown".into(),
            2 => {
                invalid.as_object_mut().unwrap().remove("selected");
            }
            3 => invalid["unexpected"] = true.into(),
            4 => invalid["targets"][&id]["schema"] = 2.into(),
            5 => invalid["targets"][&id]["config"]["bucket"] = "tampered-preview".into(),
            6 => invalid["targets"][&id]["config"]["channel"] = "production".into(),
            _ => invalid["targets"][&id]["secret_key"] = "bad\nsecret".into(),
        }
        secrets.preview_credential = serde_json::to_vec(&invalid).unwrap();
        p.credentials_save(&mut v, &secrets).unwrap();
        let before = v.items.clone();
        assert!(p.remote_settings(&v).is_err(), "case {case}");
        assert!(configure(&mut p, &mut v, AdChannel::Preview, "replacement-key").is_err());
        assert!(
            p.publish_configured_with(&root.0, AdChannel::Preview, 1001, &v, |_, _, _| panic!(
                "invalid registry must not open a source"
            ))
            .is_err()
        );
        assert_eq!(v.items, before);
    }
}

#[test]
fn retained_registry_rotation_backup_and_cross_channel_separation() {
    let (_root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "old-private-key").unwrap();
    configure(&mut p, &mut v, AdChannel::Preview, "rotated-private-key").unwrap();
    assert_eq!(p.remote_settings(&v).unwrap().targets.len(), 1);
    assert!(
        !String::from_utf8_lossy(&v.load(&p.generation).unwrap().preview_credential)
            .contains("old-private-key")
    );
    p.configure_r2(
        AdChannel::Preview,
        None,
        Zeroizing::new(String::new()),
        Zeroizing::new(String::new()),
        &mut v,
    )
    .unwrap();
    let before = v.items.clone();
    assert!(configure(&mut p, &mut v, AdChannel::Production, "rotated-private-key").is_err());
    let mut wrong = config(AdChannel::Production);
    wrong.bucket = "ads-preview".into();
    assert!(
        p.configure_r2(
            AdChannel::Production,
            Some(wrong),
            Zeroizing::new("other-key".into()),
            Zeroizing::new("other-secret".into()),
            &mut v
        )
        .is_err()
    );
    assert_eq!(v.items, before);
    let bytes = p.export_backup(b"synthetic-backup-password", &v).unwrap();
    let (_dest, mut restored, mut dest_vault) = fixture();
    restored
        .restore_backup(&bytes, b"synthetic-backup-password", &mut dest_vault)
        .unwrap();
    assert!(
        restored
            .remote_settings(&dest_vault)
            .unwrap()
            .preview
            .is_none()
    );
    restored
        .select_retained_r2(
            AdChannel::Preview,
            &config(AdChannel::Preview).identity().unwrap(),
            &mut dest_vault,
        )
        .unwrap();
    assert_eq!(
        restored.remote_settings(&dest_vault).unwrap().targets.len(),
        1
    );
    let response = serde_json::to_string(&restored.remote_settings(&dest_vault).unwrap()).unwrap();
    assert!(!response.contains("rotated-private-key") && !response.contains("secret-"));
}

#[test]
fn retained_registry_capacity_and_serialized_byte_limit_preserve_previous_vault() {
    let (_root, mut p, mut v) = fixture();
    for i in 0..super::super::TARGET_LIMIT {
        let mut c = config(AdChannel::Preview);
        c.bucket = format!("preview-{i}");
        p.configure_r2(
            AdChannel::Preview,
            Some(c),
            Zeroizing::new(format!("key-{i}")),
            Zeroizing::new("private-secret".into()),
            &mut v,
        )
        .unwrap();
    }
    let before = v.items.clone();
    assert!(configure(&mut p, &mut v, AdChannel::Preview, "overflow-key").is_err());
    assert_eq!(v.items, before);
    let mut same = config(AdChannel::Preview);
    same.bucket = "preview-0".into();
    p.configure_r2(
        AdChannel::Preview,
        Some(same),
        Zeroizing::new("rotation-at-capacity".into()),
        Zeroizing::new("rotated-secret".into()),
        &mut v,
    )
    .unwrap();
    assert_eq!(
        p.remote_settings(&v).unwrap().targets.len(),
        super::super::TARGET_LIMIT
    );
    let (_root2, mut q, mut w) = fixture();
    for i in 0..3 {
        let mut c = config(AdChannel::Preview);
        c.bucket = format!("large-preview-{i}");
        q.configure_r2(
            AdChannel::Preview,
            Some(c),
            Zeroizing::new(format!("key-{i}")),
            Zeroizing::new("s".repeat(4096)),
            &mut w,
        )
        .unwrap();
    }
    let before = w.items.clone();
    let mut c = config(AdChannel::Preview);
    c.bucket = "large-preview-overflow".into();
    assert!(
        q.configure_r2(
            AdChannel::Preview,
            Some(c),
            Zeroizing::new("overflow-key".into()),
            Zeroizing::new("s".repeat(4096)),
            &mut w
        )
        .is_err()
    );
    assert_eq!(w.items, before);
}

#[test]
fn missing_historical_target_is_reported_and_original_configuration_repairs_coverage() {
    let (root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    p.publish_configured_with(&root.0, AdChannel::Preview, 1001, &v, |c, _, _| {
        remote(&root.0, c, false)
    })
    .unwrap();
    let id = p.active_target().unwrap().to_owned();
    let mut secrets = v.load(&p.generation).unwrap();
    // A pre-registry backup can legitimately contain ledger evidence but no access material.
    secrets.preview_credential.clear();
    p.credentials_save(&mut v, &secrets).unwrap();
    assert_eq!(p.remote_settings(&v).unwrap().unavailable_targets, vec![id]);
    configure(&mut p, &mut v, AdChannel::Preview, "recovered-key").unwrap();
    assert!(
        p.remote_settings(&v)
            .unwrap()
            .unavailable_targets
            .is_empty()
    );
}
