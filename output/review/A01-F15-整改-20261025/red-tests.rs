use super::*;
use protocol::Object;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct LocalRoot(PathBuf);
impl LocalRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-publisher-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(path)
    }
}
impl Drop for LocalRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn signer() -> SigningKey {
    SigningKey::from_bytes(&[0x41; 32])
}
fn open(root: &LocalRoot) -> Publisher {
    Publisher::open(
        &root.0,
        "local-ad-key",
        Some(signer().verifying_key()),
        1000,
    )
    .unwrap()
}
#[derive(Default)]
struct MemoryVault {
    items: HashMap<String, Vec<u8>>,
    fail: bool,
}
impl Drop for MemoryVault {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        for value in self.items.values_mut() {
            value.zeroize();
        }
    }
}
impl Vault for MemoryVault {
    fn load(&self, generation: &str) -> Result<Secrets> {
        let bytes = self.items.get(generation).ok_or(Error::SecretUnavailable)?;
        signed::strict_json(bytes, security::SECRET_LIMIT).map_err(Error::Invalid)
    }
    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        if self.fail {
            return Err(Error::SecretUnavailable);
        }
        self.items
            .insert(generation.into(), serde_json::to_vec(secrets).unwrap());
        Ok(())
    }
}
fn vault(publisher: &mut Publisher) -> MemoryVault {
    let mut vault = MemoryVault::default();
    publisher
        .credentials_save(
            &mut vault,
            &Secrets {
                signing_seed: vec![0x41; 32],
                preview_credential: b"synthetic-preview-only".to_vec(),
                production_credential: b"synthetic-production-only".to_vec(),
            },
        )
        .unwrap();
    vault
}

struct MemoryStore {
    channel: AdChannel,
    objects: HashMap<String, Object>,
    commits: usize,
    conflict: bool,
    unknown_after_commit: bool,
    unknown_before_commit: bool,
    corrupt_readback: bool,
    hide_current: bool,
    next_etag: u64,
    accesses: Vec<String>,
}
impl MemoryStore {
    fn new(channel: AdChannel) -> Self {
        Self {
            channel,
            objects: HashMap::new(),
            commits: 0,
            conflict: false,
            unknown_after_commit: false,
            unknown_before_commit: false,
            corrupt_readback: false,
            hide_current: false,
            next_etag: 0,
            accesses: vec![],
        }
    }
    fn key(&self) -> String {
        protocol::current_key(self.channel)
    }
}
impl ObjectStore for MemoryStore {
    fn channel(&self) -> AdChannel {
        self.channel
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        Ok(self.objects.get(&self.key()).cloned())
    }
    fn public_read(
        &mut self,
        key: &str,
        limit: usize,
    ) -> std::result::Result<Option<Object>, StoreError> {
        self.accesses.push(key.into());
        if self.hide_current && key == self.key() {
            return Ok(None);
        }
        let mut object = self.objects.get(key).cloned();
        if object
            .as_ref()
            .is_some_and(|object| object.bytes.len() > limit)
        {
            return Err(StoreError::Unavailable);
        }
        if self.corrupt_readback
            && key.contains("/assets/")
            && let Some(object) = &mut object
        {
            object.bytes[0] ^= 1;
        }
        Ok(object)
    }
    fn put_immutable(&mut self, key: &str, bytes: &[u8]) -> std::result::Result<(), StoreError> {
        if !key.starts_with(&format!("{}/", protocol::prefix(self.channel))) {
            return Err(StoreError::Permission);
        }
        if let Some(existing) = self.objects.get(key) {
            return if existing.bytes == bytes {
                Ok(())
            } else {
                Err(StoreError::Conflict)
            };
        }
        self.objects.insert(
            key.into(),
            Object {
                bytes: bytes.into(),
                etag: "immutable".into(),
            },
        );
        Ok(())
    }
    fn commit_current(
        &mut self,
        expected_etag: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        self.commits += 1;
        if self.conflict
            || self
                .objects
                .get(&self.key())
                .map(|object| object.etag.as_str())
                != expected_etag
        {
            return Err(StoreError::Conflict);
        }
        if self.unknown_before_commit {
            self.unknown_before_commit = false;
            return Err(StoreError::OutcomeUnknown);
        }
        self.next_etag += 1;
        self.objects.insert(
            self.key(),
            Object {
                bytes: bytes.into(),
                etag: format!("etag-{}", self.next_etag),
            },
        );
        if self.unknown_after_commit {
            self.unknown_after_commit = false;
            Err(StoreError::OutcomeUnknown)
        } else {
            Ok(())
        }
    }
}

fn attach_campaign(publisher: &mut Publisher, now: u64) {
    // Minimal static 1x1 PNG, locally generated fixture without real data.
    let png: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2,
        0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 96, 96, 96, 0, 0, 0,
        4, 0, 1, 246, 23, 56, 85, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let asset = publisher.import_image(png, "image/png").unwrap();
    let mut draft = publisher.draft_read();
    draft.slots[0].campaigns = vec![switcher_domain::ads::Campaign {
        campaign_id: "synthetic-campaign".into(),
        merchant_id: "synthetic-merchant".into(),
        merchant_name: "合成商家".into(),
        title: "合成广告".into(),
        description: "不含真实资料".into(),
        image: asset,
        button_text: "查看".into(),
        target_url: "https://merchant.example.com/public-offer".into(),
        starts_at: 1000,
        ends_at: 2000,
        enabled: true,
    }];
    publisher.draft_save(draft, now).unwrap();
}

#[test]
fn strict_local_draft_atomic_lock_pause_history_and_private_images() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    assert!(
        Publisher::open(
            &root.0,
            "local-ad-key",
            Some(signer().verifying_key()),
            1000
        )
        .is_err()
    );
    let before = publisher.draft_read();
    let bytes = serde_json::to_string(&before).unwrap().replace(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
    );
    assert!(publisher.draft_save_bytes(bytes.as_bytes(), 1001).is_err());
    assert_eq!(publisher.draft_read(), before);
    attach_campaign(&mut publisher, 1001);
    let hash = publisher.draft_read().slots[0].campaigns[0]
        .image
        .sha256
        .clone();
    assert!(
        publisher
            .image_preview(&hash)
            .unwrap()
            .starts_with("data:image/png;base64,iVBORw0KGgo")
    );
    assert!(publisher.image_preview("../../outside").is_err());
    let paused = publisher
        .pause(Some(AdSlot::ResidentialBroadband), 1002)
        .unwrap();
    assert!(!paused.slots[0].enabled);
    assert!(paused.slots[1].enabled);
    assert!(paused.content_revision > before.content_revision);
    let generation = publisher.generation.clone();
    drop(publisher);
    let reopened = open(&root);
    assert_eq!(reopened.draft_read(), paused);
    assert_eq!(
        fs::metadata(root.0.join(generation).join("state.json"))
            .unwrap()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn cas_success_repeat_idempotence_pause_history_and_environment_isolation() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let vault = vault(&mut publisher);
    let mut store = MemoryStore::new(AdChannel::Production);
    let first = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1002)
        .unwrap();
    let repeated = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1003)
        .unwrap();
    assert_eq!(first, repeated);
    assert_eq!(store.commits, 1);
    assert_eq!(publisher.status().history.len(), 1);
    let paused = publisher.pause(None, 1004).unwrap();
    assert!(paused.slots.iter().all(|slot| !slot.enabled));
    let second = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1005)
        .unwrap();
    assert!(second.revision > first.revision);
    let restored = publisher.restore_history(&first.release_id, 1006).unwrap();
    assert!(restored.slots[0].enabled);
    assert!(restored.content_revision > second.revision);
    let third = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1007)
        .unwrap();
    assert!(third.revision > second.revision);
    assert_eq!(publisher.status().history.len(), 3);
    assert!(
        store
            .accesses
            .iter()
            .all(|path| path.starts_with("production/"))
    );
    let commits = store.commits;
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Preview, 1008)
            .is_err()
    );
    assert_eq!(store.commits, commits);
    assert!(matches!(
        publisher.publish(AdChannel::Production, 1009, None, &vault),
        Err(Error::MissingSource)
    ));
}

#[test]
fn cas_conflict_requires_explicit_edit_and_never_reports_success() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = MemoryStore::new(AdChannel::Preview);
    store.conflict = true;
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1001),
        Err(Error::Store(StoreError::Conflict))
    ));
    assert_eq!(publisher.status().phase, Some(Phase::Conflict));
    assert!(publisher.status().history.is_empty());
    let id = publisher.status().pending_release_id;
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Preview, 1002)
            .is_err()
    );
    assert_eq!(publisher.status().pending_release_id, id);
    publisher.draft_save(publisher.draft_read(), 1003).unwrap();
    store.conflict = false;
    publisher
        .publish_with(&mut store, &vault, AdChannel::Preview, 1004)
        .unwrap();
}

#[test]
fn unknown_result_recovers_same_release_after_restart_without_duplicate_commit() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = MemoryStore::new(AdChannel::Production);
    store.unknown_after_commit = true;
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Production, 1001),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    let id = publisher.status().pending_release_id.unwrap();
    assert!(publisher.draft_save(publisher.draft_read(), 1002).is_err());
    drop(publisher);
    let mut publisher = open(&root);
    let result = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1003)
        .unwrap();
    assert_eq!(result.release_id, id);
    assert_eq!(store.commits, 1);
}

#[test]
fn unknown_before_commit_rechecks_version_and_retries_same_id() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = MemoryStore::new(AdChannel::Preview);
    store.unknown_before_commit = true;
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Preview, 1001)
            .is_err()
    );
    let id = publisher.status().pending_release_id.unwrap();
    let result = publisher
        .publish_with(&mut store, &vault, AdChannel::Preview, 1002)
        .unwrap();
    assert_eq!(result.release_id, id);
    assert_eq!(store.commits, 2);
    assert_eq!(publisher.status().history.len(), 1);
}

#[test]
fn public_readback_must_validate_all_images_and_current_before_success() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let vault = vault(&mut publisher);
    let mut store = MemoryStore::new(AdChannel::Production);
    store.corrupt_readback = true;
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1002)
            .is_err()
    );
    assert_eq!(publisher.status().phase, Some(Phase::ReadbackFailed));
    assert!(publisher.status().history.is_empty());
    let id = publisher.status().pending_release_id.unwrap();
    store.corrupt_readback = false;
    store.hide_current = true;
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Production, 1003),
        Err(Error::Store(StoreError::Unavailable))
    ));
    assert_eq!(publisher.status().phase, Some(Phase::ReadbackFailed));
    assert!(publisher.status().history.is_empty());
    store.hide_current = false;
    assert_eq!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1004)
            .unwrap()
            .release_id,
        id
    );
    assert_eq!(store.commits, 1);
}

#[test]
fn encrypted_backup_randomness_password_parameter_limits_and_atomic_generation_restore() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let mut vault = vault(&mut publisher);
    let passphrase = b"local synthetic long passphrase";
    let backup = publisher.export_backup(passphrase, &vault).unwrap();
    let second = publisher.export_backup(passphrase, &vault).unwrap();
    assert_ne!(backup, second);
    assert!(!backup.windows(32).any(|window| window == [0x41; 32]));
    let before = publisher.generation.clone();
    let before_draft = publisher.draft_read();
    assert!(
        publisher
            .restore_backup(&backup, b"wrong synthetic passphrase", &mut vault)
            .is_err()
    );
    assert_eq!(publisher.generation, before);
    assert_eq!(publisher.draft_read(), before_draft);
    let mut malicious = backup.clone();
    malicious[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(
        backup::open(&malicious, passphrase),
        Err(Error::Invalid("backup_kdf_parameters"))
    ));
    vault.fail = true;
    assert!(
        publisher
            .restore_backup(&backup, passphrase, &mut vault)
            .is_err()
    );
    assert_eq!(publisher.generation, before);
    vault.fail = false;
    publisher
        .restore_backup(&backup, passphrase, &mut vault)
        .unwrap();
    assert_ne!(publisher.generation, before);
    assert_eq!(publisher.draft_read().slots, before_draft.slots);
    assert!(publisher.draft_read().content_revision > before_draft.content_revision);
    assert!(publisher.status().recovery_required);
    let mut preview = MemoryStore::new(AdChannel::Preview);
    assert!(matches!(
        publisher.publish_with(&mut preview, &vault, AdChannel::Preview, 1002),
        Err(Error::RecoveryReadRequired)
    ));
    let mut production = MemoryStore::new(AdChannel::Production);
    publisher
        .reconcile_production(&mut production, 1002)
        .unwrap();
    assert!(!publisher.status().recovery_required);
    publisher
        .publish_with(&mut production, &vault, AdChannel::Production, 1003)
        .unwrap();
}

#[test]
fn restore_keeps_local_authenticated_watermark_and_reuploads_missing_assets() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let mut vault = vault(&mut publisher);
    let passphrase = b"local synthetic long passphrase";
    let backup = publisher.export_backup(passphrase, &vault).unwrap();
    let mut store = MemoryStore::new(AdChannel::Production);
    let first = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1002)
        .unwrap();
    publisher.pause(None, 1003).unwrap();
    let second = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1004)
        .unwrap();
    assert!(second.revision > first.revision);
    publisher
        .restore_backup(&backup, passphrase, &mut vault)
        .unwrap();
    store.objects.retain(|key, _| !key.contains("/assets/"));
    let restored = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1005)
        .unwrap();
    assert!(restored.revision > second.revision);
    assert!(store.objects.keys().any(|key| key.contains("/assets/")));
}

#[test]
fn drafts_need_no_key_and_explicit_generation_persists_only_public_trust_on_disk() {
    let root = LocalRoot::new();
    let mut publisher = Publisher::open(&root.0, "local-ad-key", None, 1000).unwrap();
    assert!(publisher.configured_public_key().is_none());
    attach_campaign(&mut publisher, 1001);
    let mut vault = MemoryVault::default();
    let mut store = MemoryStore::new(AdChannel::Preview);
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1002),
        Err(Error::MissingSigningKey)
    ));
    assert_eq!(store.commits, 0);
    let public = publisher
        .generate_signing_key(
            &mut vault,
            b"synthetic-preview".to_vec(),
            b"synthetic-production".to_vec(),
        )
        .unwrap();
    assert_eq!(publisher.configured_public_key(), Some(public));
    let secrets = vault.load(&publisher.generation).unwrap();
    let trust = read_private(&root.0.join("trust.json"), 4096).unwrap();
    assert!(
        !trust
            .windows(32)
            .any(|window| window == secrets.signing_seed)
    );
    assert!(
        publisher
            .generate_signing_key(&mut vault, vec![], vec![])
            .is_err()
    );
    drop(publisher);
    let publisher = Publisher::open(&root.0, "local-ad-key", None, 1003).unwrap();
    assert_eq!(publisher.configured_public_key(), Some(public));
}

#[test]
fn missing_or_wrong_signing_credentials_never_create_remote_objects() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let mut vault = MemoryVault::default();
    let mut store = MemoryStore::new(AdChannel::Production);
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1001)
            .is_err()
    );
    assert!(store.objects.is_empty());
    assert!(
        publisher
            .credentials_save(
                &mut vault,
                &Secrets {
                    signing_seed: vec![0x42; 32],
                    preview_credential: vec![],
                    production_credential: b"synthetic".to_vec()
                }
            )
            .is_err()
    );
    assert!(vault.items.is_empty());
}

#[test]
fn local_store_fixed_scope_cas_immutable_private_files_and_restart() {
    use local_store::LocalObjectStore;
    use switcher_core::ad_runtime::local::{LocalTrust, directory};
    let root = LocalRoot::new();
    storage::create_private_directory(&root.0).unwrap();
    storage::create_private_directory(&root.0.join("v2")).unwrap();
    let trust = LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: signer().verifying_key().to_bytes(),
    };
    let mut store = LocalObjectStore::open(&root.0, AdChannel::Production, &trust).unwrap();
    assert!(!store.requires_credentials());
    assert!(LocalObjectStore::open(&root.0, AdChannel::Preview, &trust).is_err());
    let key = "production/versions/local-release.json";
    store.put_immutable(key, b"version").unwrap();
    store.put_immutable(key, b"version").unwrap();
    assert_eq!(
        store.put_immutable(key, b"overwrite"),
        Err(StoreError::Conflict)
    );
    assert_eq!(
        store.put_immutable("preview/versions/x.json", b"x"),
        Err(StoreError::Permission)
    );
    assert_eq!(
        store.public_read("production/../trust.json", 100).err(),
        Some(StoreError::Permission)
    );
    assert!(store.public_read(key, 2).is_err());
    store.commit_current(None, b"one").unwrap();
    assert_eq!(
        store.commit_current(None, b"two"),
        Err(StoreError::Conflict)
    );
    let etag = store.current().unwrap().unwrap().etag;
    store.commit_current(Some(&etag), b"two").unwrap();
    // A crash before exclusive rename leaves an unreachable temporary file only.
    storage::atomic_write(
        &directory(&root.0).join("production/versions/.publish-interrupted"),
        b"unused",
    )
    .unwrap();
    drop(store);
    let mut store = LocalObjectStore::open(&root.0, AdChannel::Production, &trust).unwrap();
    assert_eq!(store.current().unwrap().unwrap().bytes, b"two");
    assert_eq!(
        store.public_read(key, 100).unwrap().unwrap().bytes,
        b"version"
    );
    let bad = directory(&root.0).join("production/versions/symlink.json");
    std::os::unix::fs::symlink(directory(&root.0).join(key), bad).unwrap();
    assert!(
        store
            .public_read("production/versions/symlink.json", 100)
            .is_err()
    );
    let hard = directory(&root.0).join("production/versions/hardlink.json");
    fs::hard_link(directory(&root.0).join(key), &hard).unwrap();
    assert!(store.public_read(key, 100).is_err());
    fs::remove_file(hard).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        directory(&root.0).join(key),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(store.public_read(key, 100).is_err());
    fs::set_permissions(
        directory(&root.0).join(key),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::rename(
        directory(&root.0).join("lifecycle.lock"),
        directory(&root.0).join("old-lock"),
    )
    .unwrap();
    storage::atomic_write(&directory(&root.0).join("lifecycle.lock"), b"").unwrap();
    assert_eq!(
        store.commit_current(Some("anything"), b"replaced"),
        Err(StoreError::Permission)
    );
    let mut changed = trust.clone();
    changed.public_key = [0x42; 32];
    drop(store);
    assert!(LocalObjectStore::open(&root.0, AdChannel::Production, &changed).is_err());
}

#[test]
fn local_publication_both_editions_pause_history_backup_and_signature_verification() {
    use local_store::LocalObjectStore;
    use switcher_core::ad_runtime::{AdRuntime, local::LocalTrust};
    use switcher_domain::{Edition, ads::AdPreferences};
    let root = LocalRoot::new();
    storage::create_private_directory(&root.0).unwrap();
    storage::create_private_directory(&root.0.join("v2")).unwrap();
    let now = switcher_core::checks::unix_now();
    let mut p = Publisher::open(
        &root.0.join("drafts"),
        "ads-personal-v1",
        Some(signer().verifying_key()),
        now,
    )
    .unwrap();
    let mut vault = vault(&mut p);
    // No fabricated provider credential is needed in local mode.
    let mut secrets = vault.load(&p.generation).unwrap();
    secrets.preview_credential.clear();
    secrets.production_credential.clear();
    vault.store(&p.generation, &secrets).unwrap();
    attach_campaign(&mut p, now);
    let mut draft = p.draft_read();
    draft.slots[0].campaigns[0].starts_at = now;
    draft.slots[0].campaigns[0].ends_at = now + 3600;
    p.draft_save(draft, now).unwrap();
    let trust = LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: signer().verifying_key().to_bytes(),
    };
    let mut store = LocalObjectStore::open(&root.0, AdChannel::Production, &trust).unwrap();
    let first = p
        .publish(AdChannel::Production, now, Some(&mut store), &vault)
        .unwrap();
    let first_id = p.draft_read().release_id;
    let preferences = AdPreferences {
        slots_enabled: [true; 3],
        request_purpose_acknowledged: true,
    };
    let before_reads = local_tree(&switcher_core::ad_runtime::local::directory(&root.0));
    for edition in [Edition::Personal, Edition::Public] {
        let visible = AdRuntime::new(root.0.clone(), edition)
            .snapshot(&preferences)
            .unwrap()
            .unwrap();
        assert_eq!(visible.release_id, first_id);
        assert_eq!(visible.slots[0].campaigns.len(), 1);
    }
    assert_eq!(
        local_tree(&switcher_core::ad_runtime::local::directory(&root.0)),
        before_reads
    );
    let backup = p.export_backup(b"local-backup-password", &vault).unwrap();
    p.pause(None, now).unwrap();
    p.publish(AdChannel::Production, now, Some(&mut store), &vault)
        .unwrap();
    for edition in [Edition::Personal, Edition::Public] {
        assert!(
            AdRuntime::new(root.0.clone(), edition)
                .snapshot(&preferences)
                .unwrap()
                .is_none()
        );
    }
    p.restore_history(&first_id, now).unwrap();
    p.publish(AdChannel::Production, now, Some(&mut store), &vault)
        .unwrap();
    assert!(
        AdRuntime::new(root.0.clone(), Edition::Public)
            .snapshot(&preferences)
            .unwrap()
            .is_some()
    );
    p.restore_backup(&backup, b"local-backup-password", &mut vault)
        .unwrap();
    p.publish(AdChannel::Production, now, Some(&mut store), &vault)
        .unwrap();
    assert!(p.status().history.last().unwrap().manifest.content_revision > first.revision);
    // Restoring on a fresh installation requires no pre-existing public key.
    let fresh_root = root.0.join("restored-drafts");
    let mut fresh = Publisher::open(&fresh_root, "ads-personal-v1", None, now).unwrap();
    let before = fs::read(fresh_root.join("active.json")).unwrap();
    assert!(
        fresh
            .restore_backup(&backup, b"wrong-password", &mut vault)
            .is_err()
    );
    assert_eq!(fs::read(fresh_root.join("active.json")).unwrap(), before);
    assert!(!fresh_root.join("trust.json").exists());
    fresh
        .restore_backup(&backup, b"local-backup-password", &mut vault)
        .unwrap();
    assert!(fresh.status().recovery_required);
    fresh
        .publish(AdChannel::Production, now, Some(&mut store), &vault)
        .unwrap();
    assert!(!fresh.status().recovery_required);
    let restored_revision = fresh.draft_read().content_revision;
    drop(fresh);
    assert_eq!(
        Publisher::open(&fresh_root, "ads-personal-v1", None, now)
            .unwrap()
            .draft_read()
            .content_revision,
        restored_revision
    );
    // Preview writes are separate from the normal advertisement display channel.
    let production = store.current().unwrap().unwrap().bytes;
    drop(store);
    let mut preview = LocalObjectStore::open(&root.0, AdChannel::Preview, &trust).unwrap();
    p.publish(AdChannel::Preview, now, Some(&mut preview), &vault)
        .unwrap();
    drop(preview);
    let mut store = LocalObjectStore::open(&root.0, AdChannel::Production, &trust).unwrap();
    assert_eq!(store.current().unwrap().unwrap().bytes, production);
    // An edited local signature cannot become visible, even under private permissions.
    storage::atomic_write(
        &switcher_core::ad_runtime::local::directory(&root.0).join("production/current.json"),
        b"{}",
    )
    .unwrap();
    assert!(
        AdRuntime::new(root.0.clone(), Edition::Public)
            .snapshot(&preferences)
            .is_err()
    );
    // Failure backoff retains a diagnostic without re-parsing each UI poll.
    assert_eq!(
        AdRuntime::new(root.0.clone(), Edition::Public)
            .snapshot(&preferences)
            .unwrap_err(),
        "local_ad_previous_content_verification_failed"
    );
}

fn local_tree(path: &Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            files.extend(local_tree(&entry.path()));
        } else {
            files.push((
                entry.path(),
                signed::digest(&fs::read(entry.path()).unwrap()),
            ));
        }
    }
    files.sort();
    files
}

// Exercise the exact blocking body used by the Tauri command, injecting only
// a temporary Publisher and MemoryVault instead of DesktopState/Keychain.
#[test]
fn f15_command_body_restores_valid_33_mib_backup() {
    let source_root = LocalRoot::new();
    let mut source = open(&source_root);
    attach_campaign(&mut source, 1000);
    let source_vault = vault(&mut source);
    let passphrase = b"synthetic-backup-passphrase";
    let exported = source.export_backup(passphrase, &source_vault).unwrap();
    let mut plaintext = backup::open(&exported, passphrase).unwrap();
    // Valid JSON trailing whitespace tests wire size, not a large asset corpus.
    plaintext.resize(33 * 1024 * 1024 - 76, b' ');
    let large = backup::seal(&plaintext, passphrase).unwrap();
    assert_eq!(large.len(), 33 * 1024 * 1024);
    let target_root = LocalRoot::new();
    let mut target = open(&target_root);
    let mut target_vault = MemoryVault::default();
    let result = crate::publisher_commands::restore_publisher_backup_body(
        &large,
        String::from_utf8(passphrase.to_vec()).unwrap(),
        |bytes, password| target.restore_backup(bytes, password, &mut target_vault).map_err(|e| e.to_string()),
    );
    assert!(result.is_ok(), "valid 33 MiB backup rejected by command body: {result:?}");
    assert_eq!(target.draft_read().slots, source.draft_read().slots);
    assert_eq!(target_vault.items.len(), 1);
}
