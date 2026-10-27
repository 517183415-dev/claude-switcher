use super::test_support::MemoryVault;
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
    immutable_writes: usize,
    conflict: bool,
    unknown_after_commit: bool,
    unknown_before_commit: bool,
    corrupt_readback: bool,
    hide_current: bool,
    next_etag: u64,
    accesses: Vec<String>,
    current_reads: usize,
    fail_current: bool,
    fail_immutable: bool,
    target_identity: &'static str,
    target_after_current: Option<&'static str>,
}
impl MemoryStore {
    fn new(channel: AdChannel) -> Self {
        Self {
            channel,
            objects: HashMap::new(),
            commits: 0,
            immutable_writes: 0,
            conflict: false,
            unknown_after_commit: false,
            unknown_before_commit: false,
            corrupt_readback: false,
            hide_current: false,
            next_etag: 0,
            accesses: vec![],
            current_reads: 0,
            fail_current: false,
            fail_immutable: false,
            target_identity: "local:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            target_after_current: None,
        }
    }
    fn key(&self) -> String {
        protocol::current_key(self.channel)
    }
}
impl ObjectStore for MemoryStore {
    fn target_identity(&self) -> Option<String> {
        Some(self.target_identity.to_owned())
    }
    fn channel(&self) -> AdChannel {
        self.channel
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        self.current_reads += 1;
        if self.fail_current {
            return Err(StoreError::Unavailable);
        }
        if let Some(target) = self.target_after_current {
            self.target_identity = target;
        }
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
        self.immutable_writes += 1;
        if self.fail_immutable {
            return Err(StoreError::Unavailable);
        }
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

// The first request can still reach the source after its caller times out.
struct DelayedCommitStore {
    inner: MemoryStore,
    delayed: Option<(Option<String>, Vec<u8>)>,
    fail_put: bool,
    fail_after_conflict: bool,
    conflict_seen: bool,
    leave_delayed_in_flight: bool,
}
impl ObjectStore for DelayedCommitStore {
    fn target_identity(&self) -> Option<String> {
        Some(format!("local:{}", "a".repeat(64)))
    }
    fn channel(&self) -> AdChannel {
        self.inner.channel()
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        if self.conflict_seen && self.fail_after_conflict {
            return Err(StoreError::Unavailable);
        }
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
        if self.fail_put {
            Err(StoreError::Unavailable)
        } else {
            self.inner.put_immutable(key, bytes)
        }
    }
    fn commit_current(
        &mut self,
        etag: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        if let Some((old_etag, old_bytes)) = self.delayed.take() {
            if self.leave_delayed_in_flight {
                self.delayed = Some((old_etag, old_bytes));
                self.conflict_seen = true;
                return Err(StoreError::Conflict);
            }
            // The original request commits after recovery's read but before retry.
            self.inner.commit_current(old_etag.as_deref(), &old_bytes)?;
            let result = self.inner.commit_current(etag, bytes);
            self.conflict_seen = matches!(result, Err(StoreError::Conflict));
            result
        } else {
            self.delayed = Some((etag.map(str::to_owned), bytes.to_vec()));
            Err(StoreError::OutcomeUnknown)
        }
    }
}

// Reproduce the legacy recovery path that persisted Uploading after an
// earlier CAS returned an unknown result.
#[test]
fn legacy_uploading_delayed_commit_recovers_after_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = DelayedCommitStore {
        inner: MemoryStore::new(AdChannel::Preview),
        delayed: None,
        fail_put: false,
        fail_after_conflict: false,
        conflict_seen: false,
        leave_delayed_in_flight: false,
    };
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1001),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    let id = publisher.status().pending_release_id.unwrap();
    publisher.set_phase(Phase::Uploading).unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    let result = publisher
        .publish_with(&mut store, &vault, AdChannel::Preview, 1002)
        .unwrap();
    assert_eq!(result.release_id, id);
    assert_eq!(publisher.status().history.len(), 1);
    assert_eq!(store.inner.commits, 2);
    drop(publisher);
    assert_eq!(open(&root).status().history.len(), 1);
}

#[test]
fn legacy_uploading_upload_failure_keeps_pending_across_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = DelayedCommitStore {
        inner: MemoryStore::new(AdChannel::Preview),
        delayed: None,
        fail_put: false,
        fail_after_conflict: false,
        conflict_seen: false,
        leave_delayed_in_flight: false,
    };
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1001),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    let pending = publisher.state.pending.clone().unwrap();
    publisher.set_phase(Phase::Uploading).unwrap();
    store.fail_put = true;
    drop(publisher);
    let mut publisher = open(&root);
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1002),
        Err(Error::Store(StoreError::Unavailable))
    ));
    assert_eq!(publisher.status().phase, Some(Phase::Uploading));
    assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
    assert!(matches!(
        publisher.draft_save(publisher.draft_read(), 1003),
        Err(Error::PendingOperation)
    ));
    drop(publisher);
    let mut publisher = open(&root);
    assert_eq!(publisher.status().phase, Some(Phase::Uploading));
    store.fail_put = false;
    assert_eq!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Preview, 1004)
            .unwrap()
            .release_id,
        pending.manifest.release_id
    );
}

#[test]
fn legacy_uploading_without_version_resumes_first_upload() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = DelayedCommitStore {
        inner: MemoryStore::new(AdChannel::Preview),
        delayed: None,
        fail_put: true,
        fail_after_conflict: false,
        conflict_seen: false,
        leave_delayed_in_flight: false,
    };
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1001),
        Err(Error::Store(StoreError::Unavailable))
    ));
    let pending = publisher.state.pending.clone().unwrap();
    assert_eq!(pending.phase, Phase::Uploading);
    assert!(
        !store
            .inner
            .objects
            .contains_key(&protocol::version_key(&pending.manifest))
    );
    drop(publisher);
    store.fail_put = false;
    let mut publisher = open(&root);
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1002),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    let result = publisher
        .publish_with(&mut store, &vault, AdChannel::Preview, 1003)
        .unwrap();
    assert_eq!(result.release_id, pending.manifest.release_id);
    assert_eq!(publisher.status().history.len(), 1);
}

struct CompetingStore {
    inner: MemoryStore,
    competing_wire: Vec<u8>,
}
impl ObjectStore for CompetingStore {
    fn target_identity(&self) -> Option<String> {
        Some(format!("local:{}", "a".repeat(64)))
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
        expected: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        self.inner.commit_current(expected, &self.competing_wire)?;
        self.inner.commit_current(expected, bytes)
    }
}

#[test]
fn legacy_uploading_competing_signed_commit_preserves_pending_until_resolved() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut initial = DelayedCommitStore {
        inner: MemoryStore::new(AdChannel::Preview),
        delayed: None,
        fail_put: false,
        fail_after_conflict: false,
        conflict_seen: false,
        leave_delayed_in_flight: false,
    };
    assert!(matches!(
        publisher.publish_with(&mut initial, &vault, AdChannel::Preview, 1001),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    let pending = publisher.state.pending.clone().unwrap();
    let mut competitor = pending.manifest.clone();
    competitor.content_revision += 1;
    competitor.release_id = random_id().unwrap();
    let payload = String::from_utf8(encode(&competitor, ads::MAX_MANIFEST_BYTES).unwrap()).unwrap();
    let signature = signer().sign(payload.as_bytes()).to_bytes().to_vec();
    let competing_wire = encode(
        &signed::Envelope { payload, signature },
        ads::MAX_MANIFEST_BYTES,
    )
    .unwrap();
    publisher.set_phase(Phase::Uploading).unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    let mut store = CompetingStore {
        inner: initial.inner,
        competing_wire,
    };
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1002),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    assert_eq!(publisher.status().phase, Some(Phase::CommitUnknown));
    assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
    assert!(matches!(
        publisher.draft_save(publisher.draft_read(), 1003),
        Err(Error::PendingOperation)
    ));
    drop(publisher);
    let mut publisher = open(&root);
    assert_eq!(publisher.status().phase, Some(Phase::CommitUnknown));
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1004),
        Err(Error::Resolved(_))
    ));
    assert!(publisher.state.pending.is_none());
    assert_eq!(
        publisher.resolution_read().unwrap().kind,
        ResolutionKind::Superseded
    );
}

#[test]
fn delayed_unknown_commit_is_recognized_after_retry_conflict() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = DelayedCommitStore {
        inner: MemoryStore::new(AdChannel::Production),
        delayed: None,
        fail_put: false,
        fail_after_conflict: false,
        conflict_seen: false,
        leave_delayed_in_flight: false,
    };
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Production, 1001),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    let id = publisher.status().pending_release_id.unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    let result = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1002)
        .unwrap();
    assert_eq!(result.release_id, id);
    assert_eq!(store.inner.commits, 2); // one commit and one rejected CAS
    assert_eq!(publisher.status().history.len(), 1);
    drop(publisher);
    let publisher = open(&root);
    assert!(publisher.status().phase.is_none());
    assert_eq!(publisher.status().history.len(), 1);
}

#[test]
fn delayed_unknown_commit_remains_uncertain_when_retry_upload_fails() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut store = DelayedCommitStore {
        inner: MemoryStore::new(AdChannel::Preview),
        delayed: None,
        fail_put: false,
        fail_after_conflict: false,
        conflict_seen: false,
        leave_delayed_in_flight: false,
    };
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1001),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    store.fail_put = true;
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1002),
        Err(Error::Store(StoreError::Unavailable))
    ));
    assert_eq!(publisher.status().phase, Some(Phase::CommitUnknown));
    drop(publisher);
    let mut publisher = open(&root);
    assert_eq!(publisher.status().phase, Some(Phase::CommitUnknown));
    assert!(matches!(
        publisher.draft_save(publisher.draft_read(), 1003),
        Err(Error::PendingOperation)
    ));
}

#[test]
fn delayed_unknown_commit_never_retires_on_missing_or_failed_conflict_read() {
    for case in ["missing", "unavailable"] {
        let root = LocalRoot::new();
        let mut publisher = open(&root);
        let vault = vault(&mut publisher);
        let mut store = DelayedCommitStore {
            inner: MemoryStore::new(AdChannel::Preview),
            delayed: None,
            fail_put: false,
            fail_after_conflict: case == "unavailable",
            conflict_seen: false,
            leave_delayed_in_flight: case == "missing",
        };
        assert!(matches!(
            publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1001),
            Err(Error::Store(StoreError::OutcomeUnknown))
        ));
        let pending = publisher.state.pending.clone().unwrap();
        let result = publisher.publish_with(&mut store, &vault, AdChannel::Preview, 1002);
        assert!(matches!(
            result,
            Err(Error::Store(
                StoreError::OutcomeUnknown | StoreError::Unavailable
            ))
        ));
        assert_eq!(publisher.status().phase, Some(Phase::CommitUnknown));
        assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
        assert!(publisher.status().history.is_empty());
        assert!(matches!(
            publisher.draft_save(publisher.draft_read(), 1003),
            Err(Error::PendingOperation)
        ));
        drop(publisher);
        let publisher = open(&root);
        assert_eq!(publisher.status().phase, Some(Phase::CommitUnknown));
        assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
    }
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
        .publish_with(&mut production, &vault, AdChannel::Production, 1003)
        .unwrap();
    assert!(!publisher.status().recovery_required);
}

#[test]
fn backup_restore_rejects_orphan_vault_collision_without_changing_active_generation() {
    struct CollisionVault {
        inner: MemoryVault,
        orphan: Vec<u8>,
        collision: Option<String>,
    }
    impl Vault for CollisionVault {
        fn load(&self, generation: &str) -> Result<Secrets> {
            self.inner.load(generation)
        }
        fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
            self.inner
                .items
                .insert(generation.into(), self.orphan.clone());
            self.collision = Some(generation.into());
            self.inner.store(generation, secrets)
        }
        fn create(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
            self.inner
                .items
                .insert(generation.into(), self.orphan.clone());
            self.collision = Some(generation.into());
            self.inner.create(generation, secrets)
        }
    }

    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let mut vault = CollisionVault {
        inner: vault(&mut publisher),
        orphan: b"synthetic existing orphan credential".to_vec(),
        collision: None,
    };
    let passphrase = b"synthetic orphan collision passphrase";
    let backup = publisher.export_backup(passphrase, &vault).unwrap();
    publisher.pause(None, 1002).unwrap();
    let generation = publisher.generation.clone();
    let state = encode(&publisher.state, STATE_LIMIT).unwrap();
    let old_state = fs::read(root.0.join(&generation).join("state.json")).unwrap();
    let active = fs::read(root.0.join("active.json")).unwrap();
    let credentials = vault.inner.items.clone();

    assert!(matches!(
        publisher.restore_backup(&backup, passphrase, &mut vault),
        Err(Error::SecretUnavailable)
    ));
    let collision = vault.collision.as_ref().unwrap();
    assert_ne!(collision, &generation);
    assert_eq!(vault.inner.items.get(collision), Some(&vault.orphan));
    for (key, value) in credentials {
        assert_eq!(vault.inner.items.get(&key), Some(&value));
    }
    assert_eq!(publisher.generation, generation);
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), state);
    assert_eq!(
        fs::read(root.0.join(&generation).join("state.json")).unwrap(),
        old_state
    );
    assert_eq!(fs::read(root.0.join("active.json")).unwrap(), active);
    drop(publisher);
    let reopened = open(&root);
    assert_eq!(reopened.generation, generation);
    assert_eq!(encode(&reopened.state, STATE_LIMIT).unwrap(), state);
}

#[test]
fn backup_vault_failure_preserves_active_generation_and_credentials_after_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let mut vault = vault(&mut publisher);
    let passphrase = b"synthetic restart atomicity passphrase";
    let backup = publisher.export_backup(passphrase, &vault).unwrap();
    // Make the active draft differ from the backup, so reopening the backup
    // accidentally cannot satisfy the assertions about the old generation.
    publisher.pause(None, 1002).unwrap();
    let generation = publisher.generation.clone();
    let state = encode(&publisher.state, STATE_LIMIT).unwrap();
    let active = fs::read(root.0.join("active.json")).unwrap();
    let credentials = vault.items.clone();
    vault.fail = true;
    assert!(matches!(
        publisher.restore_backup(&backup, passphrase, &mut vault),
        Err(Error::SecretUnavailable)
    ));
    assert_eq!(publisher.generation, generation);
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), state);
    assert_eq!(fs::read(root.0.join("active.json")).unwrap(), active);
    assert_eq!(vault.items, credentials);
    drop(publisher);
    let mut reopened = open(&root);
    assert_eq!(reopened.generation, generation);
    assert_eq!(encode(&reopened.state, STATE_LIMIT).unwrap(), state);
    assert!(!reopened.status().recovery_required);
    // The old credentials remain loadable and can still sign a publication.
    let mut store = MemoryStore::new(AdChannel::Production);
    reopened
        .publish_with(&mut store, &vault, AdChannel::Production, 1003)
        .unwrap();
    assert_eq!(store.commits, 1);
}

#[test]
fn restored_backup_restart_preserves_recovery_gate_and_authenticated_watermark() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let mut vault = vault(&mut publisher);
    let passphrase = b"synthetic restored recovery restart passphrase";
    let backup = publisher.export_backup(passphrase, &vault).unwrap();
    let mut production = MemoryStore::new(AdChannel::Production);
    let published = publisher
        .publish_with(&mut production, &vault, AdChannel::Production, 1002)
        .unwrap();
    publisher.pause(None, 1003).unwrap();
    publisher
        .restore_backup(&backup, passphrase, &mut vault)
        .unwrap();
    let generation = publisher.generation.clone();
    let state = encode(&publisher.state, STATE_LIMIT).unwrap();
    assert!(publisher.status().recovery_required);
    drop(publisher);
    let mut reopened = open(&root);
    assert_eq!(reopened.generation, generation);
    assert_eq!(encode(&reopened.state, STATE_LIMIT).unwrap(), state);
    assert!(reopened.status().recovery_required);
    assert!(reopened.draft_read().content_revision > published.revision);
    let mut preview = MemoryStore::new(AdChannel::Preview);
    assert!(matches!(
        reopened.publish_with(&mut preview, &vault, AdChannel::Preview, 1004),
        Err(Error::RecoveryReadRequired)
    ));
    assert_eq!(preview.commits, 0);
    assert_eq!(preview.immutable_writes, 0);
    assert!(preview.accesses.is_empty());
    reopened
        .reconcile_production(&mut production, 1004)
        .unwrap();
    assert!(!reopened.status().recovery_required);
    let restored = reopened
        .publish_with(&mut production, &vault, AdChannel::Production, 1005)
        .unwrap();
    assert!(restored.revision > published.revision);
    assert_eq!(production.commits, 2);
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
    fn clock() -> std::result::Result<switcher_core::time::ClockSample, &'static str> {
        Ok(switcher_core::time::ClockSample {
            utc_secs: 1_700_000_000,
            continuous_ms: 1_700_000_000_000,
            boot_id: "test-boot".into(),
        })
    }
    let now = clock().unwrap().utc_secs as u64;
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
            .with_clock(clock)
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
                .with_clock(clock)
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
            .with_clock(clock)
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
            .with_clock(clock)
            .snapshot(&preferences)
            .is_err()
    );
    // Failure backoff retains a diagnostic without re-parsing each UI poll.
    assert_eq!(
        AdRuntime::new(root.0.clone(), Edition::Public)
            .with_clock(clock)
            .snapshot(&preferences)
            .unwrap_err(),
        "local_ad_previous_content_verification_failed"
    );
}

pub(super) fn local_tree(path: &Path) -> Vec<(PathBuf, String)> {
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
fn f15_restore_large_backup(wire_size: usize) {
    let source_root = LocalRoot::new();
    let mut source = open(&source_root);
    attach_campaign(&mut source, 1000);
    let source_vault = vault(&mut source);
    let source_active = fs::read(source_root.0.join("active.json")).unwrap();
    let source_generation = source.generation.clone();
    let source_secrets = source_vault.items.clone();
    let passphrase = b"synthetic-backup-passphrase";
    let exported = source.export_backup(passphrase, &source_vault).unwrap();
    let mut plaintext = backup::open(&exported, passphrase).unwrap();
    // Valid JSON trailing whitespace tests wire size, not a large asset corpus.
    plaintext.resize(wire_size - 76, b' ');
    let large = backup::seal(&plaintext, passphrase).unwrap();
    assert_eq!(large.len(), wire_size);
    let target_root = LocalRoot::new();
    let mut target = open(&target_root);
    let old_generation = target.generation.clone();
    let mut target_vault = MemoryVault::default();
    let mut called = false;
    let result = crate::publisher_commands::restore_publisher_backup_body(
        &large,
        String::from_utf8(passphrase.to_vec()).unwrap(),
        |bytes, password| {
            called = true;
            target
                .restore_backup(bytes, password, &mut target_vault)
                .map_err(|e| e.to_string())
        },
    );
    assert!(
        result.is_ok(),
        "valid {wire_size}-byte backup rejected by command body: {result:?}"
    );
    assert!(called);
    assert_ne!(target.generation, old_generation);
    assert_eq!(target.draft_read().slots, source.draft_read().slots);
    assert_eq!(target_vault.items.len(), 1);
    assert!(target_vault.items.contains_key(&target.generation));
    assert_eq!(source.generation, source_generation);
    assert_eq!(
        fs::read(source_root.0.join("active.json")).unwrap(),
        source_active
    );
    assert_eq!(source_vault.items, source_secrets);
    drop(target);
    let reopened = open(&target_root);
    assert_eq!(reopened.draft_read().slots, source.draft_read().slots);
}

#[test]
fn f15_command_body_restores_valid_33_mib_backup() {
    f15_restore_large_backup(33 * 1024 * 1024);
}

#[test]
fn f15_command_body_restores_inclusive_64_mib_backup() {
    f15_restore_large_backup(backup::LIMIT);
}

#[test]
fn f15_command_body_failures_preserve_source_target_and_vault() {
    let source_root = LocalRoot::new();
    let mut source = open(&source_root);
    attach_campaign(&mut source, 1000);
    let source_vault = vault(&mut source);
    let source_active = fs::read(source_root.0.join("active.json")).unwrap();
    let source_secrets = source_vault.items.clone();
    let password = b"synthetic-backup-passphrase";
    let encrypted = source.export_backup(password, &source_vault).unwrap();
    let target_root = LocalRoot::new();
    let mut target = open(&target_root);
    let mut target_vault = vault(&mut target);
    let target_generation = target.generation.clone();
    let target_active = fs::read(target_root.0.join("active.json")).unwrap();
    let target_draft = target.draft_read();
    let target_secrets = target_vault.items.clone();
    for case in [
        "wrong_password",
        "tampered",
        "truncated",
        "oversize",
        "vault_failure",
    ] {
        let mut bytes = match case {
            "oversize" => vec![0; backup::LIMIT + 1],
            "truncated" => encrypted[..75].to_vec(),
            _ => encrypted.clone(),
        };
        if case == "tampered" {
            *bytes.last_mut().unwrap() ^= 1;
        }
        let passphrase = if case == "wrong_password" {
            "different-synthetic-passphrase"
        } else {
            "synthetic-backup-passphrase"
        };
        target_vault.fail = case == "vault_failure";
        let mut called = false;
        let result = crate::publisher_commands::restore_publisher_backup_body(
            &bytes,
            passphrase.into(),
            |bytes, password| {
                called = true;
                target
                    .restore_backup(bytes, password, &mut target_vault)
                    .map_err(|e| e.to_string())
            },
        );
        assert!(result.is_err(), "{case} must fail");
        assert_eq!(
            called,
            case != "oversize",
            "{case}: size gate must precede the restore callback"
        );
        assert_eq!(target.generation, target_generation, "{case}");
        assert_eq!(target.draft_read(), target_draft, "{case}");
        assert_eq!(
            fs::read(target_root.0.join("active.json")).unwrap(),
            target_active,
            "{case}"
        );
        assert_eq!(target_vault.items, target_secrets, "{case}");
        assert_eq!(
            fs::read(source_root.0.join("active.json")).unwrap(),
            source_active,
            "{case}"
        );
        assert_eq!(source_vault.items, source_secrets, "{case}");
    }
}

#[test]
fn f15_backup_plaintext_budget_includes_header_and_tag() {
    let oversized = vec![b' '; backup::LIMIT - 76 + 1];
    let error = backup::seal(&oversized, b"synthetic-backup-passphrase").unwrap_err();
    assert!(error.to_string().contains("backup_size"));
}

// F16: these fixtures use only synthetic memory stores and private temporary files.
fn f16_committed_pending(with_image: bool) -> (LocalRoot, Publisher, MemoryVault, MemoryStore) {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    if with_image {
        attach_campaign(&mut publisher, 1001);
    }
    let vault = vault(&mut publisher);
    let mut store = MemoryStore::new(AdChannel::Production);
    store.unknown_after_commit = true;
    assert!(matches!(
        publisher.publish_with(&mut store, &vault, AdChannel::Production, 1002),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    (root, publisher, vault, store)
}
fn f16_wire(manifest: &AdManifest) -> Vec<u8> {
    let payload = String::from_utf8(encode(manifest, ads::MAX_MANIFEST_BYTES).unwrap()).unwrap();
    let signature = signer().sign(payload.as_bytes()).to_bytes().to_vec();
    encode(
        &signed::Envelope { payload, signature },
        ads::MAX_MANIFEST_BYTES,
    )
    .unwrap()
}
#[test]
fn f16_committed_missing_version_is_repaired_without_another_cas() {
    let (root, publisher, vault, mut store) = f16_committed_pending(false);
    let pending = publisher.state.pending.clone().unwrap();
    store
        .objects
        .remove(&protocol::version_key(&pending.manifest));
    drop(publisher);
    let mut publisher = open(&root);
    let result = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1003)
        .unwrap();
    assert_eq!(result.release_id, pending.manifest.release_id);
    assert_eq!(store.commits, 1);
    assert_eq!(
        store.objects[&protocol::version_key(&pending.manifest)].bytes,
        pending.wire
    );
}
#[test]
fn f16_committed_missing_image_is_repaired_from_local_bytes_without_cas() {
    let (_root, mut publisher, vault, mut store) = f16_committed_pending(true);
    let pending = publisher.state.pending.clone().unwrap();
    let asset = &pending.manifest.slots[0].campaigns[0].image;
    store
        .objects
        .remove(&protocol::asset_key(AdChannel::Production, asset));
    publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1003)
        .unwrap();
    assert_eq!(store.commits, 1);
}
#[test]
fn f16_committed_expired_reply_retires_pending_without_verified_history() {
    let (root, mut publisher, vault, mut store) = f16_committed_pending(false);
    let pending = publisher.state.pending.clone().unwrap();
    assert!(
        publisher
            .publish_with(
                &mut store,
                &vault,
                AdChannel::Production,
                pending.manifest.expires_at
            )
            .is_err()
    );
    assert!(publisher.state.pending.is_none());
    assert!(publisher.state.history.is_empty());
    assert_eq!(store.commits, 1);
    drop(publisher);
    assert!(open(&root).state.pending.is_none());
}
#[test]
fn f16_committed_proven_missing_image_retires_without_verified_history() {
    let (_root, mut publisher, vault, mut store) = f16_committed_pending(true);
    let pending = publisher.state.pending.clone().unwrap();
    let asset = &pending.manifest.slots[0].campaigns[0].image;
    store
        .objects
        .remove(&protocol::asset_key(AdChannel::Production, asset));
    fs::remove_file(publisher.directory().join("assets").join(&asset.sha256)).unwrap();
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1003)
            .is_err()
    );
    assert!(publisher.state.pending.is_none());
    assert!(publisher.state.history.is_empty());
    assert_eq!(store.commits, 1);
}
#[test]
fn f16_trusted_higher_current_supersedes_pending_without_remote_write() {
    let (_root, mut publisher, vault, mut store) = f16_committed_pending(false);
    let mut higher = publisher.state.pending.as_ref().unwrap().manifest.clone();
    higher.content_revision += 1;
    higher.release_id = "higher-authenticated-release".into();
    let wire = f16_wire(&higher);
    store.objects.insert(
        store.key(),
        Object {
            bytes: wire.clone(),
            etag: "higher-etag".into(),
        },
    );
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1003)
            .is_err()
    );
    assert!(publisher.state.pending.is_none());
    assert_eq!(
        publisher.state.sequences[1].highest,
        higher.content_revision
    );
    assert!(publisher.state.history.is_empty());
    assert_eq!(store.objects[&store.key()].bytes, wire);
    assert_eq!(store.commits, 1);
}
#[test]
fn f16_global_pause_removes_all_campaign_references_for_safe_higher_release() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    attach_campaign(&mut publisher, 1001);
    let vault = vault(&mut publisher);
    let hash = publisher.state.draft.slots[0].campaigns[0]
        .image
        .sha256
        .clone();
    fs::remove_file(publisher.directory().join("assets").join(hash)).unwrap();
    let paused = publisher.pause(None, 1003).unwrap();
    assert!(
        paused
            .slots
            .iter()
            .all(|slot| !slot.enabled && slot.campaigns.is_empty())
    );
    let mut store = MemoryStore::new(AdChannel::Production);
    publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1004)
        .unwrap();
}

struct F16Store {
    inner: MemoryStore,
    current: Option<std::result::Result<Option<Object>, StoreError>>,
    fail_read: Option<String>,
    fail_put: bool,
}
impl F16Store {
    fn new(inner: MemoryStore) -> Self {
        Self {
            inner,
            current: None,
            fail_read: None,
            fail_put: false,
        }
    }
}
impl ObjectStore for F16Store {
    fn target_identity(&self) -> Option<String> {
        Some(format!("local:{}", "a".repeat(64)))
    }
    fn channel(&self) -> AdChannel {
        self.inner.channel()
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        self.current.clone().unwrap_or_else(|| self.inner.current())
    }
    fn public_read(
        &mut self,
        key: &str,
        limit: usize,
    ) -> std::result::Result<Option<Object>, StoreError> {
        if self.fail_read.as_deref() == Some(key) {
            Err(StoreError::Unavailable)
        } else {
            self.inner.public_read(key, limit)
        }
    }
    fn put_immutable(&mut self, key: &str, bytes: &[u8]) -> std::result::Result<(), StoreError> {
        if self.fail_put {
            Err(StoreError::Unavailable)
        } else {
            self.inner.put_immutable(key, bytes)
        }
    }
    fn commit_current(
        &mut self,
        etag: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        self.inner.commit_current(etag, bytes)
    }
}
fn f16_expire(
    publisher: &mut Publisher,
    store: &mut MemoryStore,
    vault: &MemoryVault,
) -> ResolutionSummary {
    let now = publisher
        .state
        .pending
        .as_ref()
        .unwrap()
        .manifest
        .expires_at;
    match publisher.publish_with(store, vault, AdChannel::Production, now) {
        Err(Error::Resolved(summary)) => summary,
        result => panic!("expected explicit non-verified resolution: {result:?}"),
    }
}
#[test]
fn f16_authenticated_current_failures_cannot_be_bypassed_by_public_exact_wire() {
    for case in [
        "bad_signature",
        "future",
        "regression",
        "same_sequence",
        "missing",
        "unavailable",
        "permission",
    ] {
        let (_root, mut publisher, vault, store) = f16_committed_pending(false);
        let pending = publisher.state.pending.clone().unwrap();
        let mut store = F16Store::new(store);
        // Establish an actual authenticated watermark before the adversarial read.
        publisher.read_remote(&mut store, 1003).unwrap();
        let mut altered = pending.manifest.clone();
        let current = match case {
            "unavailable" => Err(StoreError::Unavailable),
            "permission" => Err(StoreError::Permission),
            "missing" => Ok(None),
            _ => {
                if case == "future" {
                    altered.issued_at = 1005;
                    altered.expires_at += 3;
                }
                if case == "same_sequence" {
                    altered.release_id = "different-same-sequence".into();
                }
                if case == "regression" {
                    let mut higher = altered.clone();
                    higher.content_revision += 1;
                    let (_, hash) = publisher
                        .authenticated_manifest(&f16_wire(&higher), AdChannel::Production, 1003)
                        .unwrap();
                    let mut state = publisher.state.clone();
                    state.sequences[1]
                        .remember(higher.content_revision, hash)
                        .unwrap();
                    publisher.persist(state).unwrap();
                }
                let mut wire = f16_wire(&altered);
                if case == "bad_signature" {
                    let mut env: signed::Envelope = serde_json::from_slice(&wire).unwrap();
                    env.signature[0] ^= 1;
                    wire = serde_json::to_vec(&env).unwrap();
                }
                Ok(Some(Object {
                    bytes: wire,
                    etag: "synthetic-adversarial-current".into(),
                }))
            }
        };
        store.current = Some(current);
        let before = fs::read(publisher.directory().join("state.json")).unwrap();
        let remote = store.inner.objects[&store.inner.key()].bytes.clone();
        assert!(
            publisher
                .publish_with(&mut store, &vault, AdChannel::Production, 1004)
                .is_err(),
            "{case}"
        );
        assert_eq!(
            fs::read(publisher.directory().join("state.json")).unwrap(),
            before,
            "{case}"
        );
        assert_eq!(
            publisher.state.pending.as_ref().unwrap().wire,
            pending.wire,
            "{case}"
        );
        assert_eq!(
            store.inner.objects[&store.inner.key()].bytes,
            remote,
            "{case}"
        );
        assert!(publisher.state.history.is_empty());
        assert_eq!(store.inner.commits, 1);
    }
}
#[test]
fn f16_corrupt_immutable_objects_are_never_overwritten_or_retired() {
    for kind in ["version", "image"] {
        let (_root, mut publisher, vault, mut store) = f16_committed_pending(true);
        let pending = publisher.state.pending.clone().unwrap();
        let key = if kind == "version" {
            protocol::version_key(&pending.manifest)
        } else {
            protocol::asset_key(
                AdChannel::Production,
                &pending.manifest.slots[0].campaigns[0].image,
            )
        };
        store.objects.get_mut(&key).unwrap().bytes = b"corrupt immutable".to_vec();
        assert!(
            publisher
                .publish_with(&mut store, &vault, AdChannel::Production, 1003)
                .is_err()
        );
        assert_eq!(store.objects[&key].bytes, b"corrupt immutable");
        assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
        assert!(publisher.resolution_read().is_none());
        assert!(publisher.state.history.is_empty());
        assert_eq!(store.commits, 1);
    }
}
#[test]
fn f16_unsafe_or_corrupt_local_assets_never_become_missing_asset_resolutions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    for kind in [
        "corrupt",
        "mode",
        "symlink",
        "hardlink",
        "directory",
        "index",
    ] {
        let (_root, mut publisher, vault, mut store) = f16_committed_pending(true);
        let pending = publisher.state.pending.clone().unwrap();
        let asset = &pending.manifest.slots[0].campaigns[0].image;
        store
            .objects
            .remove(&protocol::asset_key(AdChannel::Production, asset));
        let path = publisher.directory().join("assets").join(&asset.sha256);
        match kind {
            "corrupt" => fs::write(&path, b"corrupt").unwrap(),
            "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            "symlink" => {
                let other = path.with_extension("saved");
                fs::rename(&path, &other).unwrap();
                symlink(&other, &path).unwrap();
            }
            "hardlink" => fs::hard_link(&path, path.with_extension("extra")).unwrap(),
            "directory" => {
                fs::remove_file(&path).unwrap();
                fs::create_dir(&path).unwrap();
            }
            "index" => {
                publisher.state.assets.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            publisher
                .publish_with(&mut store, &vault, AdChannel::Production, 1003)
                .is_err(),
            "{kind}"
        );
        assert!(publisher.resolution_read().is_none(), "{kind}");
        assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
        assert!(publisher.state.history.is_empty());
        assert_eq!(store.commits, 1);
    }
}
#[test]
fn f16_remote_read_or_upload_errors_keep_pending_and_never_claim_resolution() {
    for kind in ["read", "put"] {
        let (_root, mut publisher, vault, mut store) = f16_committed_pending(true);
        let pending = publisher.state.pending.clone().unwrap();
        let key = protocol::asset_key(
            AdChannel::Production,
            &pending.manifest.slots[0].campaigns[0].image,
        );
        store.objects.remove(&key);
        let mut store = F16Store::new(store);
        if kind == "read" {
            store.fail_read = Some(key);
        } else {
            store.fail_put = true;
        }
        assert!(matches!(
            publisher.publish_with(&mut store, &vault, AdChannel::Production, 1003),
            Err(Error::Store(StoreError::Unavailable))
        ));
        assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
        assert!(publisher.resolution_read().is_none());
        assert_eq!(store.inner.commits, 1);
        store.fail_read = None;
        store.fail_put = false;
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1004)
            .unwrap();
        assert_eq!(store.inner.commits, 1);
    }
}
#[test]
fn f16_terminal_evidence_survives_restart_save_and_successful_higher_pause() {
    let (root, mut publisher, vault, mut store) = f16_committed_pending(true);
    let pending = publisher.state.pending.clone().unwrap();
    let summary = f16_expire(&mut publisher, &mut store, &vault);
    assert_eq!(summary.kind, ResolutionKind::ExpiredCommitted);
    let evidence = encode(&publisher.resolution_read(), STATE_LIMIT).unwrap();
    assert_eq!(
        publisher.resolution_read().unwrap().pending.wire,
        pending.wire
    );
    drop(publisher);
    let mut publisher = open(&root);
    assert_eq!(publisher.status().last_resolution, Some(summary));
    let now = pending.manifest.expires_at + 1;
    let paused = publisher.pause(None, now).unwrap();
    assert!(
        paused
            .slots
            .iter()
            .all(|slot| !slot.enabled && slot.campaigns.is_empty())
    );
    let result = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, now + 1)
        .unwrap();
    assert!(result.revision > pending.manifest.content_revision);
    assert_eq!(
        encode(&publisher.resolution_read(), STATE_LIMIT).unwrap(),
        evidence
    );
    assert_eq!(publisher.state.history.len(), 1);
    assert_eq!(store.commits, 2);
}
#[test]
fn f16_second_terminal_preserves_both_original_evidence_and_new_pending() {
    let (_root, mut publisher, vault, mut store) = f16_committed_pending(false);
    let old_time = publisher
        .state
        .pending
        .as_ref()
        .unwrap()
        .manifest
        .expires_at;
    f16_expire(&mut publisher, &mut store, &vault);
    let evidence = encode(&publisher.resolution_read(), STATE_LIMIT).unwrap();
    publisher.pause(None, old_time + 1).unwrap();
    store.unknown_after_commit = true;
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, old_time + 2)
            .is_err()
    );
    let pending = publisher.state.pending.clone().unwrap();
    assert!(matches!(
        publisher.publish_with(
            &mut store,
            &vault,
            AdChannel::Production,
            pending.manifest.expires_at
        ),
        Err(Error::ResolutionOccupied)
    ));
    assert_eq!(
        encode(&publisher.resolution_read(), STATE_LIMIT).unwrap(),
        evidence
    );
    assert_eq!(publisher.state.pending.as_ref().unwrap().wire, pending.wire);
    assert_eq!(store.commits, 2);
    assert!(publisher.state.history.is_empty());
}
#[test]
fn f16_terminal_write_failures_preserve_or_calibrate_exact_disk_commit() {
    for fault in [1, 2] {
        let (root, mut publisher, vault, mut store) = f16_committed_pending(false);
        let pending = publisher.state.pending.clone().unwrap();
        F16_TERMINAL_WRITE_FAULT.with(|slot| slot.set(fault));
        assert!(matches!(
            publisher.publish_with(
                &mut store,
                &vault,
                AdChannel::Production,
                pending.manifest.expires_at
            ),
            Err(Error::Io(_))
        ));
        let bytes = fs::read(publisher.directory().join("state.json")).unwrap();
        assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), bytes);
        assert_eq!(publisher.state.pending.is_some(), fault == 1);
        assert_eq!(publisher.resolution_read().is_some(), fault == 2);
        drop(publisher);
        let mut publisher = open(&root);
        if fault == 1 {
            f16_expire(&mut publisher, &mut store, &vault);
        }
        let evidence = publisher.resolution_read().unwrap();
        assert_eq!(evidence.pending.wire, pending.wire);
        assert_eq!(evidence.current_wire, store.objects[&store.key()].bytes);
        assert!(publisher.state.pending.is_none());
        assert!(publisher.state.history.is_empty());
        assert_eq!(store.commits, 1);
    }
}
#[test]
fn f16_old_state_without_resolution_field_remains_readable() {
    let root = LocalRoot::new();
    let publisher = open(&root);
    let path = publisher.directory().join("state.json");
    let mut state: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state.as_object_mut().unwrap().remove("last_resolution");
    storage::atomic_write(&path, &serde_json::to_vec(&state).unwrap()).unwrap();
    drop(publisher);
    assert!(open(&root).resolution_read().is_none());
}
#[test]
fn f16_resolution_tampering_fails_closed_on_reopen() {
    for field in [
        "pending_wire",
        "current_wire",
        "pending_manifest",
        "channel",
        "time",
        "kind",
        "watermark",
        "state_watermark",
        "etag",
        "assets",
    ] {
        let (root, mut publisher, vault, mut store) = f16_committed_pending(false);
        f16_expire(&mut publisher, &mut store, &vault);
        let mut state = publisher.state.clone();
        let resolution = state.last_resolution.as_mut().unwrap();
        match field {
            "pending_wire" => resolution.pending.wire[0] ^= 1,
            "current_wire" => resolution.current_wire[0] ^= 1,
            "pending_manifest" => resolution.pending.manifest.release_id.push('x'),
            "channel" => resolution.pending.manifest.channel = AdChannel::Preview,
            "time" => resolution.resolved_at = 0,
            "kind" => resolution.kind = ResolutionKind::Superseded,
            "watermark" => resolution.watermark.highest += 1,
            "state_watermark" => state.sequences[1] = Default::default(),
            "etag" => resolution.current_etag.clear(),
            "assets" => resolution.unavailable_assets.push("not-referenced".into()),
            _ => unreachable!(),
        }
        let path = publisher.directory().join("state.json");
        storage::atomic_write(&path, &encode(&state, STATE_LIMIT).unwrap()).unwrap();
        drop(publisher);
        assert!(
            Publisher::open(
                &root.0,
                "local-ad-key",
                Some(signer().verifying_key()),
                1000
            )
            .is_err(),
            "{field}"
        );
    }
}

#[test]
fn f16_retired_release_retry_after_restart_returns_same_resolution_without_cas() {
    let (root, mut publisher, vault, mut store) = f16_committed_pending(false);
    let summary = f16_expire(&mut publisher, &mut store, &vault);
    drop(publisher);
    for _ in 0..2 {
        let mut publisher = open(&root);
        assert!(
            matches!(publisher.publish_with(&mut store, &vault, AdChannel::Production, summary.resolved_at + 1), Err(Error::Resolved(actual)) if actual == summary)
        );
        assert!(publisher.state.history.is_empty());
        assert!(publisher.state.pending.is_none());
    }
    assert_eq!(store.commits, 1);
}
#[test]
fn f16_small_backup_restore_retains_current_resolution_from_older_snapshot() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let mut vault = vault(&mut publisher);
    let passphrase = b"synthetic isolated F16 passphrase";
    let backup = publisher.export_backup(passphrase, &vault).unwrap();
    let mut store = MemoryStore::new(AdChannel::Production);
    store.unknown_after_commit = true;
    assert!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1002)
            .is_err()
    );
    f16_expire(&mut publisher, &mut store, &vault);
    let evidence = encode(&publisher.resolution_read(), STATE_LIMIT).unwrap();
    publisher
        .restore_backup(&backup, passphrase, &mut vault)
        .unwrap();
    assert_eq!(
        encode(&publisher.resolution_read(), STATE_LIMIT).unwrap(),
        evidence
    );
    let backup = publisher.export_backup(passphrase, &vault).unwrap();
    publisher
        .restore_backup(&backup, passphrase, &mut vault)
        .unwrap();
    assert_eq!(
        encode(&publisher.resolution_read(), STATE_LIMIT).unwrap(),
        evidence
    );
}

#[test]
fn f16_clean_publisher_restores_resolution_using_authenticated_backup_key() {
    let (_source_root, mut source, source_vault, mut store) = f16_committed_pending(false);
    let summary = f16_expire(&mut source, &mut store, &source_vault);
    let evidence = encode(&source.resolution_read(), STATE_LIMIT).unwrap();
    let passphrase = b"synthetic isolated fresh restore passphrase";
    let backup = source.export_backup(passphrase, &source_vault).unwrap();
    let target_root = LocalRoot::new();
    let mut target = Publisher::open(&target_root.0, "local-ad-key", None, 1000).unwrap();
    let mut target_vault = MemoryVault::default();
    assert!(target.configured_public_key().is_none());
    assert!(!target_root.0.join("trust.json").exists());
    target
        .restore_backup(&backup, passphrase, &mut target_vault)
        .unwrap();
    assert_eq!(
        target.configured_public_key(),
        Some(signer().verifying_key().to_bytes())
    );
    assert_eq!(target.status().last_resolution, Some(summary));
    assert_eq!(
        encode(&target.resolution_read(), STATE_LIMIT).unwrap(),
        evidence
    );
    drop(target);
    let reopened = Publisher::open(&target_root.0, "local-ad-key", None, 1000).unwrap();
    assert_eq!(
        encode(&reopened.resolution_read(), STATE_LIMIT).unwrap(),
        evidence
    );
}

#[test]
fn f15_valid_image_accepts_exact_byte_and_pixel_limits() {
    use base64::Engine;
    // Generated locally: 2000x1000 static, one-bit grayscale pixels, no real data.
    let mut png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAB9AAAAPoAQAAAACuJVgHAAABCklEQVR42u3BAQ0AAADCoPdPbQ8HFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwLcB1KUAAeq0Fz8AAAAASUVORK5CYII=")
        .unwrap();
    let end = png.split_off(png.len() - 12);
    // A valid private ancillary PNG chunk fills the source to the inclusive byte limit.
    let padding_len = ads::MAX_IMAGE_BYTES - png.len() - end.len() - 12;
    png.extend_from_slice(&(padding_len as u32).to_be_bytes());
    let checksum_start = png.len();
    png.extend_from_slice(b"npAD");
    png.resize(png.len() + padding_len, 0);
    let mut crc = u32::MAX;
    for byte in &png[checksum_start..] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    png.extend_from_slice(&(!crc).to_be_bytes());
    png.extend_from_slice(&end);
    assert_eq!(png.len(), ads::MAX_IMAGE_BYTES);
    let source = ImageAsset {
        sha256: signed::digest(&png),
        mime: "image/png".into(),
        bytes: png.len() as u32,
        width: 2000,
        height: 1000,
    };
    assert_eq!(
        u64::from(source.width) * u64::from(source.height),
        ads::MAX_IMAGE_PIXELS
    );
    ads::validate_image(&png, &source).unwrap();
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let imported = publisher.import_image(&png, "image/png").unwrap();
    assert_eq!(
        (imported.width, imported.height),
        (source.width, source.height)
    );
    let normalized = publisher.image_bytes(&imported).unwrap();
    assert_eq!(normalized.len(), imported.bytes as usize);
    assert!(
        normalized.len() < png.len(),
        "normalization removes ancillary padding"
    );
    ads::validate_image(&normalized, &imported).unwrap();
    let before = local_tree(&root.0);
    png.push(0);
    assert!(publisher.import_image(&png, "image/png").is_err());
    assert_eq!(before, local_tree(&root.0));
}

fn six_campaign_draft(publisher: &mut Publisher) -> AdManifest {
    attach_campaign(publisher, 1001);
    let mut draft = publisher.draft_read();
    let template = draft.slots[0].campaigns[0].clone();
    for (slot_index, slot) in draft.slots.iter_mut().enumerate() {
        slot.campaigns = (0..2)
            .map(|period| {
                let mut campaign = template.clone();
                campaign.campaign_id = format!("synthetic-slot-{slot_index}-period-{period}");
                campaign.title = format!("合成广告 {slot_index}/{period}");
                campaign.starts_at = 1000 + period * 1000;
                campaign.ends_at = campaign.starts_at + 1000;
                campaign
            })
            .collect();
    }
    publisher.draft_save(draft, 1002).unwrap()
}

#[test]
fn branch_audit_six_campaign_edits_pauses_and_failed_save_survive_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let complete = six_campaign_draft(&mut publisher);
    assert_eq!(complete.slots.len(), 3);
    assert!(complete.slots.iter().all(|slot| slot.campaigns.len() == 2));
    drop(publisher);
    let mut publisher = open(&root);
    assert_eq!(publisher.draft_read(), complete);

    for index in 0..3 {
        let before = publisher.draft_read();
        let mut edited = before.clone();
        edited.slots[index].campaigns[1].title = format!("已编辑的合成广告 {index}");
        let saved = publisher.draft_save(edited.clone(), 1010).unwrap();
        assert_eq!(saved.slots, edited.slots);
        for other in (0..3).filter(|other| *other != index) {
            assert_eq!(saved.slots[other], before.slots[other]);
        }
        let paused = publisher
            .pause(Some(saved.slots[index].slot), 1011)
            .unwrap();
        let mut expected = saved.slots.clone();
        expected[index].enabled = false;
        assert_eq!(paused.slots, expected);
        assert!(paused.content_revision > saved.content_revision);
        drop(publisher);
        publisher = open(&root);
        assert_eq!(publisher.draft_read(), paused);

        let state = encode(&publisher.state, STATE_LIMIT).unwrap();
        let files = local_tree(&root.0);
        let mut invalid = paused.clone();
        invalid.slots[index].campaigns[1].starts_at = invalid.slots[index].campaigns[0].starts_at;
        assert!(publisher.draft_save(invalid, 1012).is_err());
        assert_eq!(publisher.draft_read(), paused);
        assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), state);
        assert_eq!(local_tree(&root.0), files);
        drop(publisher);
        publisher = open(&root);
        assert_eq!(publisher.draft_read(), paused);
    }
}

#[test]
fn branch_audit_history_restore_restart_and_republish_keep_strict_sequence() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let complete = six_campaign_draft(&mut publisher);
    let vault = vault(&mut publisher);
    let mut store = MemoryStore::new(AdChannel::Production);
    let first = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1003)
        .unwrap();
    publisher.pause(Some(complete.slots[1].slot), 1004).unwrap();
    let second = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1005)
        .unwrap();
    assert!(second.revision > first.revision);
    let restored = publisher.restore_history(&first.release_id, 1006).unwrap();
    assert_eq!(restored.slots, complete.slots);
    assert!(restored.content_revision > second.revision);
    assert_ne!(restored.release_id, first.release_id);
    drop(publisher);
    let mut publisher = open(&root);
    assert_eq!(publisher.draft_read(), restored);
    let third = publisher
        .publish_with(&mut store, &vault, AdChannel::Production, 1007)
        .unwrap();
    assert_eq!(third.revision, restored.content_revision);
    assert!(third.revision > second.revision);
    assert_eq!(third.release_id, restored.release_id);
    let (remote, _): (AdManifest, String) = signed::verify(
        &store.objects[&store.key()].bytes,
        &signer().verifying_key(),
        ads::MAX_MANIFEST_BYTES,
    )
    .unwrap();
    assert_eq!(remote, publisher.draft_read());
    assert_eq!(remote.slots, complete.slots);
    drop(publisher);
    let mut publisher = open(&root);
    let history = publisher.status().history;
    assert_eq!(history.len(), 3);
    assert_eq!(
        history
            .iter()
            .map(|record| record.manifest.content_revision)
            .collect::<Vec<_>>(),
        vec![first.revision, second.revision, third.revision]
    );
    assert_eq!(
        publisher
            .publish_with(&mut store, &vault, AdChannel::Production, 1008)
            .unwrap(),
        third
    );
    assert_eq!(store.commits, 3);
}

fn memory_store_state(store: &MemoryStore) -> Vec<(String, Vec<u8>, String)> {
    let mut objects: Vec<_> = store
        .objects
        .iter()
        .map(|(key, object)| (key.clone(), object.bytes.clone(), object.etag.clone()))
        .collect();
    objects.sort();
    objects
}

#[test]
fn branch_audit_single_channel_credentials_deny_only_other_channel_without_writes() {
    // This is the credential-requiring ObjectStore composition contract. The
    // current local filesystem adapter requires no provider credential.
    for available in [AdChannel::Preview, AdChannel::Production] {
        let root = LocalRoot::new();
        let mut publisher = open(&root);
        six_campaign_draft(&mut publisher);
        let mut vault = vault(&mut publisher);
        let mut preview = MemoryStore::new(AdChannel::Preview);
        let mut production = MemoryStore::new(AdChannel::Production);
        publisher
            .publish_with(&mut preview, &vault, AdChannel::Preview, 1003)
            .unwrap();
        publisher
            .publish_with(&mut production, &vault, AdChannel::Production, 1004)
            .unwrap();
        let mut draft = publisher.draft_read();
        draft.slots[0].campaigns[0].title = "凭据矩阵的新合成版本".into();
        publisher.draft_save(draft, 1005).unwrap();
        let mut secrets = vault.load(&publisher.generation).unwrap();
        if available == AdChannel::Preview {
            secrets.production_credential.clear();
        } else {
            secrets.preview_credential.clear();
        }
        publisher.credentials_save(&mut vault, &secrets).unwrap();
        let vault_before = vault.items.clone();
        let (allowed, denied) = if available == AdChannel::Preview {
            (&mut preview, &mut production)
        } else {
            (&mut production, &mut preview)
        };
        let denied_objects = memory_store_state(denied);
        let denied_writes = denied.immutable_writes;
        let denied_commits = denied.commits;
        let allowed_commits = allowed.commits;
        publisher
            .publish_with(allowed, &vault, available, 1006)
            .unwrap();
        assert_eq!(allowed.commits, allowed_commits + 1);
        let allowed_objects = memory_store_state(allowed);
        let allowed_writes = allowed.immutable_writes;
        let files = local_tree(&root.0);
        let draft = publisher.draft_read();
        let history = encode(&publisher.state, STATE_LIMIT).unwrap();
        let denied_channel = denied.channel;
        let error = publisher
            .publish_with(denied, &vault, denied_channel, 1007)
            .unwrap_err();
        assert!(matches!(error, Error::SecretUnavailable));
        let diagnostic = format!("{error} {error:?}");
        for marker in ["synthetic-preview-only", "synthetic-production-only"] {
            assert!(!diagnostic.contains(marker));
        }
        assert_eq!(memory_store_state(denied), denied_objects);
        assert_eq!(denied.immutable_writes, denied_writes);
        assert_eq!(denied.commits, denied_commits);
        assert_eq!(memory_store_state(allowed), allowed_objects);
        assert_eq!(allowed.immutable_writes, allowed_writes);
        assert_eq!(allowed.commits, allowed_commits + 1);
        assert_eq!(publisher.draft_read(), draft);
        assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), history);
        assert_eq!(local_tree(&root.0), files);
        assert_eq!(vault.items, vault_before);
        drop(publisher);
        assert_eq!(open(&root).draft_read(), draft);
    }
}

// Exercise process exit around the real local CAS, without production fault hooks.
struct ExitingLocalStore {
    inner: local_store::LocalObjectStore,
    exit_at: Option<String>,
    receipt: PathBuf,
    commits: usize,
    allowed_commits: usize,
    immutable_writes: Vec<String>,
}
impl ObjectStore for ExitingLocalStore {
    fn target_identity(&self) -> Option<String> {
        self.inner.target_identity()
    }
    fn channel(&self) -> AdChannel {
        self.inner.channel()
    }
    fn requires_credentials(&self) -> bool {
        self.inner.requires_credentials()
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
        self.inner.put_immutable(key, bytes)?;
        self.immutable_writes.push(key.into());
        Ok(())
    }
    fn commit_current(
        &mut self,
        etag: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        if self.exit_at.as_deref() == Some("before") {
            storage::atomic_write(&self.receipt, b"before:0").unwrap();
            std::process::exit(73);
        }
        assert!(
            self.commits < self.allowed_commits,
            "unexpected repeated local CAS"
        );
        self.inner.commit_current(etag, bytes)?;
        self.commits += 1;
        if self.exit_at.as_deref() == Some("after") {
            storage::atomic_write(&self.receipt, b"after:1").unwrap();
            std::process::exit(73);
        }
        Ok(())
    }
}
fn exit_test_publisher(root: &Path) -> Publisher {
    Publisher::open(
        root,
        "ads-personal-v1",
        Some(signer().verifying_key()),
        1000,
    )
    .unwrap()
}
fn exit_test_trust() -> switcher_core::ad_runtime::local::LocalTrust {
    switcher_core::ad_runtime::local::LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: signer().verifying_key().to_bytes(),
    }
}
#[test]
fn local_store_cas_exit_child() {
    let Some(path) = std::env::var_os("SWITCHER_LOCAL_CAS_EXIT_ROOT") else {
        return;
    };
    let root = PathBuf::from(path);
    assert!(root.is_absolute());
    let cut = std::env::var("SWITCHER_LOCAL_CAS_EXIT_AT").unwrap();
    assert!(matches!(cut.as_str(), "before" | "after"));
    let drafts = LocalRoot(root.join("drafts"));
    let mut publisher = exit_test_publisher(&drafts.0);
    attach_campaign(&mut publisher, 1001);
    let vault = vault(&mut publisher);
    let mut source = ExitingLocalStore {
        inner: local_store::LocalObjectStore::open(
            &root,
            AdChannel::Production,
            &exit_test_trust(),
        )
        .unwrap(),
        exit_at: Some(cut),
        receipt: root.join("exit-receipt"),
        commits: 0,
        allowed_commits: 1,
        immutable_writes: vec![],
    };
    let _ = publisher.publish_with(&mut source, &vault, AdChannel::Production, 1002);
    panic!("local CAS exit point was not reached");
}
fn local_store_exit_recovery(cut: &str, missing: Option<&str>) {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let root = LocalRoot::new();
    storage::create_private_directory(&root.0).unwrap();
    for directory in ["home", "tmp", "v2"] {
        storage::create_private_directory(&root.0.join(directory)).unwrap();
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "publisher::tests::local_store_cas_exit_child"])
        .env_clear()
        .env("HOME", root.0.join("home"))
        .env("TMPDIR", root.0.join("tmp"))
        .env("SWITCHER_LOCAL_CAS_EXIT_ROOT", &root.0)
        .env("SWITCHER_LOCAL_CAS_EXIT_AT", cut)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("controlled local CAS child exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(root.0.join("exit-receipt")).unwrap(),
        format!("{cut}:{}", usize::from(cut == "after")).as_bytes()
    );
    // Both real OS locks must be available after exit, without Rust Drop.
    let drafts = LocalRoot(root.0.join("drafts"));
    let mut publisher = exit_test_publisher(&drafts.0);
    assert_eq!(publisher.status().phase, Some(Phase::CommitUnknown));
    assert!(publisher.status().history.is_empty());
    let pending = publisher.state.pending.clone().unwrap();
    let mut source = ExitingLocalStore {
        inner: local_store::LocalObjectStore::open(
            &root.0,
            AdChannel::Production,
            &exit_test_trust(),
        )
        .unwrap(),
        exit_at: None,
        receipt: root.0.join("unused-receipt"),
        commits: 0,
        allowed_commits: usize::from(cut == "before"),
        immutable_writes: vec![],
    };
    let initial = source.current().unwrap();
    if cut == "after" {
        assert_eq!(initial.as_ref().unwrap().bytes, pending.wire);
    } else {
        assert!(initial.is_none());
    }
    let current_path = switcher_core::ad_runtime::local::directory(&root.0)
        .join(protocol::current_key(AdChannel::Production));
    let current_metadata = initial
        .as_ref()
        .map(|_| fs::metadata(&current_path).unwrap());
    let removed = missing.map(|kind| {
        assert_eq!(cut, "after");
        let key = match kind {
            "version" => protocol::version_key(&pending.manifest),
            "image" => protocol::asset_key(
                AdChannel::Production,
                &pending.manifest.slots[0].campaigns[0].image,
            ),
            _ => unreachable!(),
        };
        let original = source
            .public_read(&key, ads::MAX_IMAGE_BYTES)
            .unwrap()
            .unwrap();
        let path = switcher_core::ad_runtime::local::directory(&root.0).join(&key);
        fs::remove_file(&path).unwrap();
        assert!(!path.exists());
        assert!(
            source
                .public_read(&key, ads::MAX_IMAGE_BYTES)
                .unwrap()
                .is_none()
        );
        assert_eq!(source.current().unwrap().unwrap().bytes, pending.wire);
        (key, original)
    });
    // Recovery must not need signing secrets, even before the CAS.
    let vault = MemoryVault::default();
    let recovered = publisher
        .publish_with(&mut source, &vault, AdChannel::Production, 1003)
        .unwrap();
    assert_eq!(recovered.release_id, pending.manifest.release_id);
    assert_eq!(recovered.revision, pending.manifest.content_revision);
    assert_eq!(source.commits, usize::from(cut == "before"));
    if let Some((key, original)) = removed {
        assert_eq!(source.immutable_writes, vec![key.clone()]);
        let repaired = source
            .public_read(&key, ads::MAX_IMAGE_BYTES)
            .unwrap()
            .unwrap();
        assert_eq!(repaired.bytes, original.bytes);
        assert_eq!(repaired.etag, original.etag);
    } else if cut == "after" {
        assert!(source.immutable_writes.is_empty());
    }
    if let Some(before) = current_metadata {
        let after = fs::metadata(&current_path).unwrap();
        assert_eq!(after.ino(), before.ino());
        assert_eq!(after.mtime(), before.mtime());
        assert_eq!(after.mtime_nsec(), before.mtime_nsec());
    }
    let current = source.current().unwrap().unwrap();
    assert_eq!(current.bytes, pending.wire);
    if let Some(initial) = initial {
        assert_eq!(current.etag, initial.etag);
    }
    let (manifest, _): (AdManifest, String) = signed::verify(
        &current.bytes,
        &signer().verifying_key(),
        ads::MAX_MANIFEST_BYTES,
    )
    .unwrap();
    assert_eq!(manifest, pending.manifest);
    assert_eq!(
        source
            .public_read(&protocol::version_key(&manifest), ads::MAX_MANIFEST_BYTES)
            .unwrap()
            .unwrap()
            .bytes,
        current.bytes
    );
    for campaign in manifest.slots.iter().flat_map(|slot| &slot.campaigns) {
        let image = source
            .public_read(
                &protocol::asset_key(AdChannel::Production, &campaign.image),
                ads::MAX_IMAGE_BYTES,
            )
            .unwrap()
            .unwrap();
        ads::validate_image(&image.bytes, &campaign.image).unwrap();
    }
    assert!(publisher.status().phase.is_none());
    assert_eq!(publisher.status().history.len(), 1);
    assert_eq!(publisher.status().history[0].manifest, manifest);
    drop(source);
    drop(publisher);
    let reopened = exit_test_publisher(&drafts.0);
    assert!(reopened.status().phase.is_none());
    assert_eq!(reopened.status().history.len(), 1);
    assert_eq!(reopened.status().history[0].manifest, manifest);
}

#[test]
fn local_store_cas_process_exit_recovers_same_release_before_and_after_commit() {
    for cut in ["before", "after"] {
        local_store_exit_recovery(cut, None);
    }
}
#[test]
fn local_store_cas_exit_repairs_missing_version_and_image_without_recommitting() {
    for missing in ["version", "image"] {
        local_store_exit_recovery("after", Some(missing));
    }
}

// Independent sources must never consume each other's durable transaction.
#[test]
fn target_ledgers_survive_switches_and_restart_with_pending_transactions() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut a = MemoryStore::new(AdChannel::Preview);
    let mut b = MemoryStore::new(AdChannel::Preview);
    b.target_identity = "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    publisher.select_target(&a).unwrap();
    publisher
        .publish_with(&mut a, &vault, AdChannel::Preview, 1002)
        .unwrap();
    publisher.pause(None, 1003).unwrap();
    a.unknown_before_commit = true;
    assert!(
        publisher
            .publish_with(&mut a, &vault, AdChannel::Preview, 1004)
            .is_err()
    );
    let pending = publisher.state.pending.clone().unwrap();

    publisher.select_target(&b).unwrap();
    assert!(publisher.state.pending.is_none());
    assert!(publisher.state.history.is_empty());
    assert_eq!(publisher.state.sequences[0].highest, 0);
    publisher
        .publish_with(&mut b, &vault, AdChannel::Preview, 1005)
        .unwrap();
    let b_history = encode(&publisher.state.history, STATE_LIMIT).unwrap();
    drop(publisher);

    let mut publisher = open(&root);
    publisher.select_target(&a).unwrap();
    assert_eq!(
        encode(&publisher.state.pending, STATE_LIMIT).unwrap(),
        encode(&Some(pending), STATE_LIMIT).unwrap()
    );
    publisher
        .publish_with(&mut a, &vault, AdChannel::Preview, 1006)
        .unwrap();
    publisher.select_target(&b).unwrap();
    assert_eq!(
        encode(&publisher.state.history, STATE_LIMIT).unwrap(),
        b_history
    );
    assert!(publisher.state.pending.is_none());
}

#[test]
fn target_ledgers_capacity_rejects_new_target_without_changing_state() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let a = MemoryStore::new(AdChannel::Preview);
    let mut b = MemoryStore::new(AdChannel::Preview);
    b.target_identity = "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    publisher.select_target(&a).unwrap();
    let mut full = publisher.state.clone();
    for value in 0..(TARGET_LIMIT - 1) {
        full.other_targets
            .insert(format!("r2:{value:064x}"), TargetLedger::default());
    }
    publisher.persist(full).unwrap();
    let before = read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap();
    assert!(matches!(
        publisher.select_target(&b),
        Err(Error::Invalid("publisher_target_capacity"))
    ));
    assert_eq!(
        read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );
}

#[test]
fn target_ledgers_publish_remote_and_resume_without_replaying_into_local() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut remote = MemoryStore::new(AdChannel::Preview);
    remote.target_identity = "r2:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    assert!(matches!(
        publisher.publish_with(&mut remote, &vault, AdChannel::Preview, 1001),
        Err(Error::Invalid("publisher_target_unavailable"))
    ));
    assert!(publisher.state.pending.is_none());
    assert_eq!(remote.commits, 0);
    publisher.select_target(&remote).unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    publisher
        .publish_with(&mut remote, &vault, AdChannel::Preview, 1002)
        .unwrap();
    assert_eq!(remote.commits, 1);
    publisher.pause(None, 1003).unwrap();
    remote.unknown_before_commit = true;
    assert!(
        publisher
            .publish_with(&mut remote, &vault, AdChannel::Preview, 1004)
            .is_err()
    );
    let pending = publisher.state.pending.clone().unwrap();
    let remote_attempts = remote.commits;
    let mut local = MemoryStore::new(AdChannel::Preview);
    publisher.select_target(&local).unwrap();
    assert!(matches!(
        publisher.publish_with(&mut remote, &vault, AdChannel::Preview, 1005),
        Err(Error::Invalid("publisher_target_unavailable"))
    ));
    assert_eq!(remote.commits, remote_attempts);
    publisher
        .publish_with(&mut local, &vault, AdChannel::Preview, 1005)
        .unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    publisher.select_target(&remote).unwrap();
    assert_eq!(
        publisher
            .state
            .pending
            .as_ref()
            .unwrap()
            .manifest
            .release_id,
        pending.manifest.release_id
    );
    publisher
        .publish_with(&mut remote, &vault, AdChannel::Preview, 1006)
        .unwrap();
    assert_eq!(remote.commits, remote_attempts + 1);
    assert_eq!(local.commits, 1);
}

#[test]
fn target_ledgers_reject_unknown_legacy_state_and_tampered_saved_targets() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut a = MemoryStore::new(AdChannel::Preview);
    publisher
        .publish_with(&mut a, &vault, AdChannel::Preview, 1002)
        .unwrap();
    let mut legacy = publisher.state.clone();
    legacy.schema = 1;
    legacy.target = None;
    publisher.persist(legacy).unwrap();
    let before = read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap();
    let mut b = MemoryStore::new(AdChannel::Preview);
    b.target_identity = "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert!(matches!(
        publisher.select_target(&b),
        Err(Error::Invalid("legacy_target_unknown"))
    ));
    assert_eq!(
        read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );

    let mut invalid = publisher.state.clone();
    invalid.schema = 3;
    invalid.target = Some(a.target_identity.to_owned());
    invalid
        .other_targets
        .insert("r2:INVALID".into(), TargetLedger::default());
    assert!(publisher.persist(invalid).is_err());
    assert_eq!(
        read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );
}

#[test]
fn shared_draft_advances_past_all_target_watermarks() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut a = MemoryStore::new(AdChannel::Preview);
    let mut b = MemoryStore::new(AdChannel::Preview);
    b.target_identity = "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    publisher.select_target(&a).unwrap();
    publisher
        .publish_with(&mut a, &vault, AdChannel::Preview, 1002)
        .unwrap();
    publisher.pause(None, 1003).unwrap();
    publisher
        .publish_with(&mut a, &vault, AdChannel::Preview, 1004)
        .unwrap();
    let high = publisher.state.sequences[0].highest;
    publisher.select_target(&b).unwrap();
    let mut older_draft = publisher.state.clone();
    older_draft.draft.content_revision = 1;
    publisher.persist(older_draft).unwrap();
    let draft = publisher.draft_read();
    assert!(publisher.draft_save(draft, 1005).unwrap().content_revision > high);
}

#[test]
fn multi_target_backup_merges_publication_history_and_watermarks_after_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let mut vault = vault(&mut publisher);
    let mut a = MemoryStore::new(AdChannel::Preview);
    let mut b = MemoryStore::new(AdChannel::Preview);
    b.target_identity = "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    publisher.select_target(&a).unwrap();
    publisher
        .publish_with(&mut a, &vault, AdChannel::Preview, 1002)
        .unwrap();
    publisher.select_target(&b).unwrap();
    publisher
        .publish_with(&mut b, &vault, AdChannel::Preview, 1003)
        .unwrap();
    let backup = publisher
        .export_backup(b"synthetic-backup-password", &vault)
        .unwrap();
    publisher.select_target(&a).unwrap();
    attach_campaign(&mut publisher, 1004);
    let newer = publisher
        .publish_with(&mut a, &vault, AdChannel::Preview, 1005)
        .unwrap();
    let a_history = encode(&publisher.state.history, STATE_LIMIT).unwrap();
    publisher.select_target(&b).unwrap();
    let before_generation = publisher.generation.clone();
    publisher
        .restore_backup(&backup, b"synthetic-backup-password", &mut vault)
        .unwrap();
    assert_ne!(publisher.generation, before_generation);
    assert!(publisher.state.recovery_required);
    assert_eq!(
        publisher.state.other_targets[a.target_identity].sequences[0].highest,
        newer.revision
    );
    assert_eq!(
        encode(
            &publisher.state.other_targets[a.target_identity].history,
            STATE_LIMIT
        )
        .unwrap(),
        a_history
    );
    assert!(publisher.state.other_targets[a.target_identity].recovery_required);
    assert!(publisher.state.draft.content_revision > newer.revision);
    assert!(!publisher.state.assets.is_empty());
    let generation = publisher.generation.clone();
    drop(publisher);
    let mut publisher = open(&root);
    assert_eq!(publisher.generation, generation);
    assert_eq!(
        encode(
            &publisher.state.other_targets[a.target_identity].history,
            STATE_LIMIT
        )
        .unwrap(),
        a_history
    );
    assert!(
        publisher
            .state
            .assets
            .iter()
            .all(|asset| publisher.image_bytes(asset).is_ok())
    );
    publisher.select_target(&a).unwrap();
    a.channel = AdChannel::Production;
    publisher.reconcile_production(&mut a, 1006).unwrap();
    a.channel = AdChannel::Preview;
    assert!(
        publisher
            .publish_with(&mut a, &vault, AdChannel::Preview, 1007)
            .unwrap()
            .revision
            > newer.revision
    );
    publisher.select_target(&b).unwrap();
    b.channel = AdChannel::Production;
    publisher.reconcile_production(&mut b, 1008).unwrap();
    b.channel = AdChannel::Preview;
    publisher
        .publish_with(&mut b, &vault, AdChannel::Preview, 1009)
        .unwrap();
}

#[test]
fn multi_target_backup_conflicting_watermark_preserves_local_generation_and_vault() {
    let root_a = LocalRoot::new();
    let root_b = LocalRoot::new();
    let mut a = open(&root_a);
    let mut b = open(&root_b);
    let mut vault_a = vault(&mut a);
    let vault_b = vault(&mut b);
    let mut store_a = MemoryStore::new(AdChannel::Preview);
    let mut store_b = MemoryStore::new(AdChannel::Preview);
    a.select_target(&store_a).unwrap();
    b.select_target(&store_b).unwrap();
    let left = a
        .publish_with(&mut store_a, &vault_a, AdChannel::Preview, 1002)
        .unwrap();
    let right = b
        .publish_with(&mut store_b, &vault_b, AdChannel::Preview, 1002)
        .unwrap();
    assert_eq!(left.revision, right.revision);
    assert_ne!(
        a.state.sequences[0].payload_hash,
        b.state.sequences[0].payload_hash
    );
    let backup = b
        .export_backup(b"synthetic-backup-password", &vault_b)
        .unwrap();
    let before = read_private(&a.directory().join("state.json"), STATE_LIMIT).unwrap();
    let active = read_private(&root_a.0.join("active.json"), 4096).unwrap();
    let secrets = vault_a.items.clone();
    assert!(matches!(
        a.restore_backup(&backup, b"synthetic-backup-password", &mut vault_a),
        Err(Error::Invalid("backup_sequence_conflict"))
    ));
    assert_eq!(
        read_private(&a.directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );
    assert_eq!(
        read_private(&root_a.0.join("active.json"), 4096).unwrap(),
        active
    );
    assert_eq!(vault_a.items, secrets);
}

#[test]
fn multi_target_backup_conflicting_inactive_pending_is_preserved() {
    let root_a = LocalRoot::new();
    let root_b = LocalRoot::new();
    let mut a = open(&root_a);
    let mut b = open(&root_b);
    let mut vault_a = vault(&mut a);
    let vault_b = vault(&mut b);
    let mut store_a = MemoryStore::new(AdChannel::Preview);
    let mut store_b = MemoryStore::new(AdChannel::Preview);
    let mut other = MemoryStore::new(AdChannel::Preview);
    other.target_identity =
        "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    a.select_target(&store_a).unwrap();
    b.select_target(&store_b).unwrap();
    store_a.unknown_before_commit = true;
    store_b.unknown_before_commit = true;
    assert!(
        a.publish_with(&mut store_a, &vault_a, AdChannel::Preview, 1002)
            .is_err()
    );
    assert!(
        b.publish_with(&mut store_b, &vault_b, AdChannel::Preview, 1002)
            .is_err()
    );
    a.select_target(&other).unwrap();
    b.select_target(&other).unwrap();
    let backup = b
        .export_backup(b"synthetic-backup-password", &vault_b)
        .unwrap();
    let before = read_private(&a.directory().join("state.json"), STATE_LIMIT).unwrap();
    let secrets = vault_a.items.clone();
    assert!(matches!(
        a.restore_backup(&backup, b"synthetic-backup-password", &mut vault_a),
        Err(Error::PendingOperation)
    ));
    assert_eq!(
        read_private(&a.directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );
    assert_eq!(vault_a.items, secrets);
}

#[test]
fn multi_target_backup_credential_conflict_preserves_generation_and_secrets() {
    let root_a = LocalRoot::new();
    let root_b = LocalRoot::new();
    let mut a = open(&root_a);
    let mut b = open(&root_b);
    let mut vault_a = vault(&mut a);
    let mut vault_b = vault(&mut b);
    let store = MemoryStore::new(AdChannel::Preview);
    a.select_target(&store).unwrap();
    b.select_target(&store).unwrap();
    b.credentials_save(
        &mut vault_b,
        &Secrets {
            signing_seed: vec![0x41; 32],
            preview_credential: b"different-synthetic-credential".to_vec(),
            production_credential: b"synthetic-production-only".to_vec(),
        },
    )
    .unwrap();
    let backup = b
        .export_backup(b"synthetic-backup-password", &vault_b)
        .unwrap();
    let before = read_private(&a.directory().join("state.json"), STATE_LIMIT).unwrap();
    let active = read_private(&root_a.0.join("active.json"), 4096).unwrap();
    let secrets = vault_a.items.clone();
    assert!(matches!(
        a.restore_backup(&backup, b"synthetic-backup-password", &mut vault_a),
        Err(Error::Invalid("backup_credential_conflict"))
    ));
    assert_eq!(
        read_private(&a.directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );
    assert_eq!(
        read_private(&root_a.0.join("active.json"), 4096).unwrap(),
        active
    );
    assert_eq!(vault_a.items, secrets);
    drop(a);
    assert_eq!(
        read_private(&open(&root_a).directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );
}

#[test]
fn multi_target_backup_unbound_existing_credentials_cannot_be_overwritten() {
    let root_a = LocalRoot::new();
    let root_b = LocalRoot::new();
    let mut a = open(&root_a);
    let mut b = open(&root_b);
    let mut vault_a = vault(&mut a);
    let mut vault_b = vault(&mut b);
    b.select_target(&MemoryStore::new(AdChannel::Preview))
        .unwrap();
    b.credentials_save(
        &mut vault_b,
        &Secrets {
            signing_seed: vec![0x41; 32],
            preview_credential: b"different-synthetic-credential".to_vec(),
            production_credential: b"synthetic-production-only".to_vec(),
        },
    )
    .unwrap();
    let backup = b
        .export_backup(b"synthetic-backup-password", &vault_b)
        .unwrap();
    let generation = a.generation.clone();
    let secrets = vault_a.items.clone();
    assert!(matches!(
        a.restore_backup(&backup, b"synthetic-backup-password", &mut vault_a),
        Err(Error::Invalid("backup_credential_conflict"))
    ));
    assert_eq!(a.generation, generation);
    assert_eq!(vault_a.items, secrets);
}

#[test]
fn multi_target_backup_preserves_vault_item_without_trust_file() {
    let root_a = LocalRoot::new();
    let root_b = LocalRoot::new();
    let mut a = open(&root_a);
    let mut b = open(&root_b);
    let mut vault_a = MemoryVault::default();
    vault_a
        .store(
            &a.generation,
            &Secrets {
                signing_seed: vec![0x41; 32],
                preview_credential: b"existing-credential".to_vec(),
                production_credential: vec![],
            },
        )
        .unwrap();
    assert!(!root_a.0.join("trust.json").exists());
    let vault_b = vault(&mut b);
    b.select_target(&MemoryStore::new(AdChannel::Preview))
        .unwrap();
    let backup = b
        .export_backup(b"synthetic-backup-password", &vault_b)
        .unwrap();
    let generation = a.generation.clone();
    let saved = vault_a.items.clone();
    assert!(matches!(
        a.restore_backup(&backup, b"synthetic-backup-password", &mut vault_a),
        Err(Error::Invalid("backup_credential_conflict"))
    ));
    assert_eq!(a.generation, generation);
    assert_eq!(vault_a.items, saved);
}

#[test]
fn multi_target_backup_vault_read_failure_is_not_treated_as_missing_item() {
    struct Unavailable;
    impl Vault for Unavailable {
        fn load(&self, _: &str) -> Result<Secrets> {
            Err(Error::SecretUnavailable)
        }
        fn store(&mut self, _: &str, _: &Secrets) -> Result<()> {
            panic!("vault failure must stop before writing")
        }
        fn create(&mut self, _: &str, _: &Secrets) -> Result<()> {
            panic!("vault failure must stop before creating")
        }
    }
    let source_root = LocalRoot::new();
    let target_root = LocalRoot::new();
    let mut source = open(&source_root);
    let source_vault = vault(&mut source);
    source
        .select_target(&MemoryStore::new(AdChannel::Preview))
        .unwrap();
    let backup = source
        .export_backup(b"synthetic-backup-password", &source_vault)
        .unwrap();
    let mut target = open(&target_root);
    let generation = target.generation.clone();
    assert!(matches!(
        target.restore_backup(&backup, b"synthetic-backup-password", &mut Unavailable),
        Err(Error::SecretUnavailable)
    ));
    assert_eq!(target.generation, generation);
}

#[test]
fn multi_target_backup_restores_into_fresh_publisher_without_credentials() {
    let source_root = LocalRoot::new();
    let target_root = LocalRoot::new();
    let mut source = open(&source_root);
    let source_vault = vault(&mut source);
    source
        .select_target(&MemoryStore::new(AdChannel::Preview))
        .unwrap();
    let backup = source
        .export_backup(b"synthetic-backup-password", &source_vault)
        .unwrap();
    let mut target = open(&target_root);
    let mut target_vault = MemoryVault::default();
    target
        .restore_backup(&backup, b"synthetic-backup-password", &mut target_vault)
        .unwrap();
    assert_eq!(target.state.target, source.state.target);
    assert!(target.state.recovery_required);
    assert!(target_vault.load(&target.generation).is_ok());
    let generation = target.generation.clone();
    drop(target);
    assert_eq!(open(&target_root).generation, generation);
}

#[test]
fn multi_target_backup_vault_failure_preserves_active_generation_after_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let mut vault = vault(&mut publisher);
    let store = MemoryStore::new(AdChannel::Preview);
    publisher.select_target(&store).unwrap();
    let backup = publisher
        .export_backup(b"synthetic-backup-password", &vault)
        .unwrap();
    let generation = publisher.generation.clone();
    let active = read_private(&root.0.join("active.json"), 4096).unwrap();
    let secrets = vault.items.clone();
    vault.fail = true;
    assert!(
        publisher
            .restore_backup(&backup, b"synthetic-backup-password", &mut vault)
            .is_err()
    );
    assert_eq!(publisher.generation, generation);
    assert_eq!(
        read_private(&root.0.join("active.json"), 4096).unwrap(),
        active
    );
    assert_eq!(vault.items, secrets);
    drop(publisher);
    assert_eq!(open(&root).generation, generation);
}

#[test]
fn target_ledgers_block_unsafe_restore_and_cleanup() {
    struct Currents;
    impl cleanup::FixedCurrents for Currents {
        fn read_currents(&mut self) -> std::result::Result<[Option<Object>; 2], StoreError> {
            panic!("cleanup must stop before accessing an incomplete target set")
        }
    }
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let a = MemoryStore::new(AdChannel::Preview);
    let mut b = MemoryStore::new(AdChannel::Preview);
    b.target_identity = "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    publisher.select_target(&a).unwrap();
    publisher.select_target(&b).unwrap();
    let before = read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap();
    assert!(matches!(
        publisher.cleanup_preview(&mut Currents, cleanup::CleanupOptions::default(), 1002),
        Err(Error::Invalid("multi_target_cleanup_not_supported"))
    ));
    assert_eq!(
        read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap(),
        before
    );
}

#[test]
fn pending_publication_blocks_other_channel_before_source_access() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut preview = MemoryStore::new(AdChannel::Preview);
    preview.unknown_before_commit = true;
    assert!(
        publisher
            .publish_with(&mut preview, &vault, AdChannel::Preview, 1002)
            .is_err()
    );
    let state_path = publisher.directory().join("state.json");
    let before = read_private(&state_path, STATE_LIMIT).unwrap();
    drop(publisher);

    let mut publisher = open(&root);
    let mut production = MemoryStore::new(AdChannel::Production);
    assert!(matches!(
        publisher.publish_with(&mut production, &vault, AdChannel::Production, 1003),
        Err(Error::PendingOperation)
    ));
    assert_eq!(production.current_reads, 0);
    assert_eq!(production.immutable_writes, 0);
    assert_eq!(production.commits, 0);
    assert_eq!(read_private(&state_path, STATE_LIMIT).unwrap(), before);

    publisher
        .publish_with(&mut preview, &vault, AdChannel::Preview, 1004)
        .unwrap();
}

#[test]
fn target_isolation_rejects_ambiguous_cross_source_pending_before_io() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source_a = MemoryStore::new(AdChannel::Preview);
    source_a.unknown_before_commit = true;
    assert!(
        publisher
            .publish_with(&mut source_a, &vault, AdChannel::Preview, 1002)
            .is_err()
    );
    drop(publisher);
    let mut publisher = open(&root);
    let mut source_b = MemoryStore::new(AdChannel::Preview);
    source_b.target_identity =
        "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert!(
        publisher
            .publish_with(&mut source_b, &vault, AdChannel::Preview, 1003)
            .is_err()
    );
    assert_eq!(
        source_b.current_reads, 0,
        "unknown target must be refused before source read"
    );
    assert_eq!(source_b.immutable_writes, 0);
    assert_eq!(source_b.commits, 0);
}

#[test]
fn target_isolation_rejects_other_empty_source_before_io_after_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source_a = MemoryStore::new(AdChannel::Preview);
    publisher
        .publish_with(&mut source_a, &vault, AdChannel::Preview, 1002)
        .unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    let mut source_b = MemoryStore::new(AdChannel::Preview);
    source_b.target_identity =
        "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert!(
        publisher
            .publish_with(&mut source_b, &vault, AdChannel::Preview, 1003)
            .is_err()
    );
    assert_eq!(
        source_b.current_reads, 0,
        "foreign source must be refused before source read"
    );
    assert_eq!(source_b.immutable_writes, 0);
    assert_eq!(source_b.commits, 0);
}

#[test]
fn target_isolation_original_source_survives_restart() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source = MemoryStore::new(AdChannel::Preview);
    publisher
        .publish_with(&mut source, &vault, AdChannel::Preview, 1002)
        .unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    assert!(
        publisher
            .publish_with(&mut source, &vault, AdChannel::Preview, 1003)
            .is_ok()
    );
}

#[test]
fn target_isolation_legacy_local_state_upgrades_and_old_reader_rejects_it() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct LegacyState {
        schema: u32,
        draft: AdManifest,
        history: Vec<HistoryRecord>,
        assets: Vec<ImageAsset>,
        sequences: [signed::Sequence; 2],
        pending: Option<Pending>,
        recovery_required: bool,
        last_resolution: Option<Resolution>,
        cleanup: Option<cleanup::CleanupBatch>,
    }
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source = MemoryStore::new(AdChannel::Preview);
    let original_draft = publisher.draft_read();
    publisher
        .publish_with(&mut source, &vault, AdChannel::Preview, 1002)
        .unwrap();
    assert_eq!(publisher.state.schema, 2);
    assert_eq!(
        publisher.state.target.as_deref(),
        Some("local:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(publisher.draft_read().slots, original_draft.slots);
    let bytes = read_private(&publisher.directory().join("state.json"), STATE_LIMIT).unwrap();
    assert!(signed::strict_json::<LegacyState>(&bytes, STATE_LIMIT).is_err());
    drop(publisher);
    assert_eq!(open(&root).state.history.len(), 1);
}

#[test]
fn target_isolation_legacy_unknown_pending_is_preserved_without_io() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source = MemoryStore::new(AdChannel::Preview);
    source.unknown_before_commit = true;
    assert!(
        publisher
            .publish_with(&mut source, &vault, AdChannel::Preview, 1002)
            .is_err()
    );
    let mut legacy = publisher.state.clone();
    legacy.schema = 1;
    legacy.target = None;
    publisher.persist(legacy).unwrap();
    let path = publisher.directory().join("state.json");
    let before = read_private(&path, STATE_LIMIT).unwrap();
    drop(publisher);
    let mut publisher = open(&root);
    let mut local = MemoryStore::new(AdChannel::Preview);
    assert!(matches!(
        publisher.publish_with(&mut local, &vault, AdChannel::Preview, 1003),
        Err(Error::Invalid("legacy_pending_target_unknown"))
    ));
    assert_eq!(local.current_reads, 0);
    assert_eq!(read_private(&path, STATE_LIMIT).unwrap(), before);
}

#[test]
fn target_isolation_distinct_local_roots_must_not_resume_same_pending() {
    struct Interrupted(local_store::LocalObjectStore);
    impl ObjectStore for Interrupted {
        fn target_identity(&self) -> Option<String> {
            self.0.target_identity()
        }
        fn channel(&self) -> AdChannel {
            self.0.channel()
        }
        fn requires_credentials(&self) -> bool {
            false
        }
        fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
            self.0.current()
        }
        fn public_read(
            &mut self,
            key: &str,
            limit: usize,
        ) -> std::result::Result<Option<Object>, StoreError> {
            self.0.public_read(key, limit)
        }
        fn put_immutable(&mut self, _: &str, _: &[u8]) -> std::result::Result<(), StoreError> {
            Err(StoreError::Unavailable)
        }
        fn commit_current(
            &mut self,
            _: Option<&str>,
            _: &[u8],
        ) -> std::result::Result<(), StoreError> {
            unreachable!()
        }
    }
    let root = LocalRoot::new();
    storage::create_private_directory(&root.0).unwrap();
    let mut publisher = exit_test_publisher(&root.0.join("publisher"));
    let vault = vault(&mut publisher);
    let first = root.0.join("first");
    let second = root.0.join("second");
    storage::create_private_directory(&first).unwrap();
    storage::create_private_directory(&second).unwrap();
    storage::create_private_directory(&first.join("v2")).unwrap();
    storage::create_private_directory(&second.join("v2")).unwrap();
    let trust = exit_test_trust();
    let mut source_a = Interrupted(
        local_store::LocalObjectStore::open(&first, AdChannel::Preview, &trust).unwrap(),
    );
    assert!(
        publisher
            .publish_with(&mut source_a, &vault, AdChannel::Preview, 1002)
            .is_err()
    );
    drop(source_a);
    let mut source_b =
        local_store::LocalObjectStore::open(&second, AdChannel::Preview, &trust).unwrap();
    assert!(
        publisher
            .publish_with(&mut source_b, &vault, AdChannel::Preview, 1003)
            .is_err(),
        "a different local root must not commit the pending publication"
    );
    assert!(source_b.current().unwrap().is_none());
}

#[test]
fn target_isolation_replaced_local_directory_is_rejected_after_restart() {
    let root = LocalRoot::new();
    storage::create_private_directory(&root.0).unwrap();
    let app = root.0.join("app");
    storage::create_private_directory(&app).unwrap();
    storage::create_private_directory(&app.join("v2")).unwrap();
    let mut publisher = exit_test_publisher(&root.0.join("publisher"));
    let vault = vault(&mut publisher);
    let trust = exit_test_trust();
    let mut source = local_store::LocalObjectStore::open(&app, AdChannel::Preview, &trust).unwrap();
    publisher
        .publish_with(&mut source, &vault, AdChannel::Preview, 1002)
        .unwrap();
    drop(source);
    drop(publisher);
    let directory = switcher_core::ad_runtime::local::directory(&app);
    fs::rename(&directory, app.join("v2/saved-local-content")).unwrap();
    let mut replacement =
        local_store::LocalObjectStore::open(&app, AdChannel::Preview, &trust).unwrap();
    let mut publisher = exit_test_publisher(&root.0.join("publisher"));
    assert!(matches!(
        publisher.publish_with(&mut replacement, &vault, AdChannel::Preview, 1003),
        Err(Error::Invalid("publisher_target_mismatch"))
    ));
    assert!(replacement.current().unwrap().is_none());
}

#[test]
fn target_isolation_every_legacy_pending_phase_stays_unchanged() {
    for phase in [
        Phase::Prepared,
        Phase::Uploading,
        Phase::CommitUnknown,
        Phase::Conflict,
        Phase::ReadbackFailed,
        Phase::Published,
    ] {
        let root = LocalRoot::new();
        let mut publisher = open(&root);
        let vault = vault(&mut publisher);
        let mut source = MemoryStore::new(AdChannel::Preview);
        source.unknown_before_commit = true;
        assert!(
            publisher
                .publish_with(&mut source, &vault, AdChannel::Preview, 1002)
                .is_err()
        );
        let mut legacy = publisher.state.clone();
        legacy.schema = 1;
        legacy.target = None;
        legacy.pending.as_mut().unwrap().phase = phase;
        publisher.persist(legacy).unwrap();
        let path = publisher.directory().join("state.json");
        let before = read_private(&path, STATE_LIMIT).unwrap();
        drop(publisher);
        let mut publisher = open(&root);
        let mut source = MemoryStore::new(AdChannel::Preview);
        assert!(matches!(
            publisher.publish_with(&mut source, &vault, AdChannel::Preview, 1003),
            Err(Error::Invalid("legacy_pending_target_unknown"))
        ));
        assert_eq!(source.current_reads, 0);
        assert_eq!(read_private(&path, STATE_LIMIT).unwrap(), before);
    }
}

#[test]
fn restore_rejects_same_revision_with_different_authenticated_payloads() {
    for channel in [AdChannel::Preview, AdChannel::Production] {
        let root_a = LocalRoot::new();
        let root_b = LocalRoot::new();
        let mut publisher_a = open(&root_a);
        let mut publisher_b = open(&root_b);
        let mut vault_a = vault(&mut publisher_a);
        let vault_b = vault(&mut publisher_b);
        let mut source_a = MemoryStore::new(channel);
        let mut source_b = MemoryStore::new(channel);
        let a = publisher_a
            .publish_with(&mut source_a, &vault_a, channel, 1002)
            .unwrap();
        let b = publisher_b
            .publish_with(&mut source_b, &vault_b, channel, 1002)
            .unwrap();
        assert_eq!(a.revision, b.revision);
        let index = channel_index(channel);
        assert_ne!(
            publisher_a.state.sequences[index].payload_hash,
            publisher_b.state.sequences[index].payload_hash
        );
        let passphrase = b"synthetic-backup-password";
        let backup = publisher_b.export_backup(passphrase, &vault_b).unwrap();
        let generation = publisher_a.generation.clone();
        let active_before = read_private(&root_a.0.join("active.json"), 4096).unwrap();
        let before =
            read_private(&publisher_a.directory().join("state.json"), STATE_LIMIT).unwrap();
        let vault_before = vault_a.items.clone();
        assert!(matches!(
            publisher_a.restore_backup(&backup, passphrase, &mut vault_a),
            Err(Error::Invalid("backup_sequence_conflict"))
        ));
        assert_eq!(publisher_a.generation, generation);
        assert_eq!(
            read_private(&publisher_a.directory().join("state.json"), STATE_LIMIT).unwrap(),
            before
        );
        assert_eq!(vault_a.items, vault_before);
        assert_eq!(
            read_private(&root_a.0.join("active.json"), 4096).unwrap(),
            active_before
        );
        drop(publisher_a);
        assert_eq!(open(&root_a).generation, generation);
    }
}

#[test]
fn target_isolation_backup_from_different_target_cannot_replace_state_or_vault() {
    let root_a = LocalRoot::new();
    let mut publisher_a = open(&root_a);
    let vault_a = vault(&mut publisher_a);
    let mut source_a = MemoryStore::new(AdChannel::Preview);
    publisher_a
        .publish_with(&mut source_a, &vault_a, AdChannel::Preview, 1002)
        .unwrap();
    let backup = publisher_a
        .export_backup(b"synthetic passphrase", &vault_a)
        .unwrap();
    let root_b = LocalRoot::new();
    let mut publisher_b = open(&root_b);
    let mut vault_b = vault(&mut publisher_b);
    let mut source_b = MemoryStore::new(AdChannel::Preview);
    source_b.target_identity =
        "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    publisher_b
        .publish_with(&mut source_b, &vault_b, AdChannel::Preview, 1002)
        .unwrap();
    let before_state = encode(&publisher_b.state, STATE_LIMIT).unwrap();
    let before_generation = publisher_b.generation.clone();
    let before_vault = vault_b.items.clone();
    assert!(
        publisher_b
            .restore_backup(&backup, b"synthetic passphrase", &mut vault_b)
            .is_err()
    );
    assert_eq!(
        encode(&publisher_b.state, STATE_LIMIT).unwrap(),
        before_state
    );
    assert_eq!(publisher_b.generation, before_generation);
    assert_eq!(vault_b.items, before_vault);
}

#[test]
fn target_isolation_cleanup_rejects_unidentified_source_before_read() {
    struct UnknownCurrents {
        reads: usize,
    }
    impl cleanup::FixedCurrents for UnknownCurrents {
        fn read_currents(&mut self) -> std::result::Result<[Option<Object>; 2], StoreError> {
            self.reads += 1;
            Ok([None, None])
        }
    }
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source = MemoryStore::new(AdChannel::Preview);
    publisher
        .publish_with(&mut source, &vault, AdChannel::Preview, 1002)
        .unwrap();
    let mut unknown = UnknownCurrents { reads: 0 };
    assert!(
        publisher
            .cleanup_preview(&mut unknown, cleanup::CleanupOptions::default(), 1003)
            .is_err()
    );
    assert_eq!(unknown.reads, 0);
}

#[test]
fn target_isolation_legacy_evidence_blocks_cleanup_and_gc_before_source_access() {
    struct UnknownCurrents {
        reads: usize,
    }
    impl cleanup::FixedCurrents for UnknownCurrents {
        fn read_currents(&mut self) -> std::result::Result<[Option<Object>; 2], StoreError> {
            self.reads += 1;
            Ok([None, None])
        }
    }
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source = MemoryStore::new(AdChannel::Preview);
    publisher
        .publish_with(&mut source, &vault, AdChannel::Preview, 1002)
        .unwrap();
    let mut legacy = publisher.state.clone();
    legacy.schema = 1;
    legacy.target = None;
    publisher.persist(legacy).unwrap();
    let mut other = UnknownCurrents { reads: 0 };
    assert!(matches!(
        publisher.cleanup_preview(&mut other, cleanup::CleanupOptions::default(), 1003),
        Err(Error::Invalid("legacy_cleanup_target_unknown"))
    ));
    assert!(matches!(
        publisher.gc_preview(&mut other, 1003),
        Err(Error::Invalid("legacy_cleanup_target_unknown"))
    ));
    assert_eq!(other.reads, 0);
}

#[test]
fn target_isolation_legacy_history_must_not_bind_an_unverified_root() {
    let root = LocalRoot::new();
    let mut publisher = open(&root);
    let vault = vault(&mut publisher);
    let mut source_a = MemoryStore::new(AdChannel::Preview);
    publisher
        .publish_with(&mut source_a, &vault, AdChannel::Preview, 1002)
        .unwrap();
    let mut legacy = publisher.state.clone();
    legacy.schema = 1;
    legacy.target = None;
    publisher.persist(legacy).unwrap();
    let path = publisher.directory().join("state.json");
    let before = read_private(&path, STATE_LIMIT).unwrap();
    let mut source_b = MemoryStore::new(AdChannel::Preview);
    source_b.target_identity =
        "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert!(
        publisher
            .publish_with(&mut source_b, &vault, AdChannel::Preview, 1003)
            .is_err()
    );
    assert_eq!(read_private(&path, STATE_LIMIT).unwrap(), before);
    assert_eq!(source_b.current_reads, 0);
    assert!(matches!(
        publisher.publish_with(&mut source_a, &vault, AdChannel::Preview, 1003),
        Err(Error::Invalid("legacy_target_unknown"))
    ));
    assert_eq!(read_private(&path, STATE_LIMIT).unwrap(), before);
}

#[test]
fn target_isolation_draft_only_backup_fresh_restore_can_reconcile_production() {
    let root_a = LocalRoot::new();
    let mut publisher_a = open(&root_a);
    let vault_a = vault(&mut publisher_a);
    let backup = publisher_a
        .export_backup(b"synthetic passphrase", &vault_a)
        .unwrap();
    let root_b = LocalRoot::new();
    let mut publisher_b = open(&root_b);
    let mut vault_b = vault(&mut publisher_b);
    publisher_b
        .restore_backup(&backup, b"synthetic passphrase", &mut vault_b)
        .unwrap();
    assert!(publisher_b.state.recovery_required);
    assert!(publisher_b.state.target.is_none());
    drop(publisher_b);
    let mut publisher_b = open(&root_b);
    let mut preview = MemoryStore::new(AdChannel::Preview);
    assert!(matches!(
        publisher_b.publish_with(&mut preview, &vault_b, AdChannel::Preview, 1002),
        Err(Error::RecoveryReadRequired)
    ));
    let mut production = MemoryStore::new(AdChannel::Production);
    assert!(
        publisher_b
            .publish_with(&mut production, &vault_b, AdChannel::Production, 1002)
            .is_ok()
    );
    assert!(production.current_reads > 0);
    assert_eq!(production.commits, 1);
}

#[test]
fn target_isolation_draft_recovery_backup_can_restore_into_bound_target_and_repeat() {
    let root = LocalRoot::new();
    let mut p = open(&root);
    let mut v = vault(&mut p);
    let original = p.export_backup(b"synthetic passphrase", &v).unwrap();
    p.restore_backup(&original, b"synthetic passphrase", &mut v)
        .unwrap();
    let pending_backup = p.export_backup(b"synthetic passphrase", &v).unwrap();
    p.restore_backup(&pending_backup, b"synthetic passphrase", &mut v)
        .unwrap();
    drop(p);
    let mut p = open(&root);
    assert!(p.state.recovery_required);
    let mut production = MemoryStore::new(AdChannel::Production);
    p.publish_with(&mut production, &v, AdChannel::Production, 1002)
        .unwrap();
    p.restore_backup(&pending_backup, b"synthetic passphrase", &mut v)
        .unwrap();
    drop(p);
    let mut p = open(&root);
    assert!(p.state.recovery_required);
    assert!(p.state.target.is_some());
    let mut preview = MemoryStore::new(AdChannel::Preview);
    assert!(matches!(
        p.publish_with(&mut preview, &v, AdChannel::Preview, 1003),
        Err(Error::RecoveryReadRequired)
    ));
    p.publish_with(&mut production, &v, AdChannel::Production, 1003)
        .unwrap();
    assert!(!p.state.recovery_required);
}

#[test]
fn target_isolation_draft_recovery_backup_restores_into_bound_target() {
    let a = LocalRoot::new();
    let mut p = open(&a);
    let mut v = vault(&mut p);
    let original = p.export_backup(b"synthetic passphrase", &v).unwrap();
    p.restore_backup(&original, b"synthetic passphrase", &mut v)
        .unwrap();
    let backup = p.export_backup(b"synthetic passphrase", &v).unwrap();
    let b = LocalRoot::new();
    let mut target = open(&b);
    let mut target_vault = vault(&mut target);
    let mut production = MemoryStore::new(AdChannel::Production);
    target
        .publish_with(&mut production, &target_vault, AdChannel::Production, 1002)
        .unwrap();
    target
        .restore_backup(&backup, b"synthetic passphrase", &mut target_vault)
        .unwrap();
    drop(target);
    let mut target = open(&b);
    assert!(target.state.recovery_required);
    assert!(target.state.target.is_some());
    assert!(!target.state.draft_only_recovery);
    target
        .publish_with(&mut production, &target_vault, AdChannel::Production, 1003)
        .unwrap();
    assert!(!target.state.recovery_required);
}

#[test]
fn target_isolation_draft_recovery_failed_read_does_not_bind_and_can_retry() {
    let root = LocalRoot::new();
    let mut p = open(&root);
    let mut v = vault(&mut p);
    let backup = p.export_backup(b"synthetic passphrase", &v).unwrap();
    p.restore_backup(&backup, b"synthetic passphrase", &mut v)
        .unwrap();
    let before = encode(&p.state, STATE_LIMIT).unwrap();
    let mut bad = MemoryStore::new(AdChannel::Production);
    bad.fail_current = true;
    assert!(
        p.publish_with(&mut bad, &v, AdChannel::Production, 1002)
            .is_err()
    );
    assert_eq!(encode(&p.state, STATE_LIMIT).unwrap(), before);
    assert_eq!(bad.immutable_writes, 0);
    assert_eq!(bad.commits, 0);
    bad.fail_current = false;
    bad.target_after_current =
        Some("local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert!(
        p.publish_with(&mut bad, &v, AdChannel::Production, 1002)
            .is_err()
    );
    assert_eq!(encode(&p.state, STATE_LIMIT).unwrap(), before);
    drop(p);
    let mut p = open(&root);
    assert!(p.state.recovery_required);
    assert!(p.state.target.is_none());
    let mut good = MemoryStore::new(AdChannel::Production);
    good.target_identity = "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    p.publish_with(&mut good, &v, AdChannel::Production, 1003)
        .unwrap();
    let mut preview = MemoryStore::new(AdChannel::Preview);
    preview.target_identity = good.target_identity;
    p.publish_with(&mut preview, &v, AdChannel::Preview, 1004)
        .unwrap();
}

#[test]
fn target_isolation_legacy_evidence_cannot_merge_bound_backup() {
    let root_a = LocalRoot::new();
    let mut a = open(&root_a);
    let mut va = vault(&mut a);
    let mut source_a = MemoryStore::new(AdChannel::Preview);
    a.publish_with(&mut source_a, &va, AdChannel::Preview, 1002)
        .unwrap();
    let mut legacy = a.state.clone();
    legacy.schema = 1;
    legacy.target = None;
    a.persist(legacy).unwrap();
    let root_b = LocalRoot::new();
    let mut b = open(&root_b);
    let vb = vault(&mut b);
    let mut source_b = MemoryStore::new(AdChannel::Preview);
    source_b.target_identity =
        "local:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    b.publish_with(&mut source_b, &vb, AdChannel::Preview, 1002)
        .unwrap();
    let backup = b.export_backup(b"synthetic passphrase", &vb).unwrap();
    let before = encode(&a.state, STATE_LIMIT).unwrap();
    let generation = a.generation.clone();
    let credentials = va.items.clone();
    assert!(
        a.restore_backup(&backup, b"synthetic passphrase", &mut va)
            .is_err()
    );
    assert_eq!(encode(&a.state, STATE_LIMIT).unwrap(), before);
    assert_eq!(a.generation, generation);
    assert_eq!(va.items, credentials);
}

#[test]
fn target_isolation_draft_recovery_authenticates_before_binding_and_persists_watermark() {
    let remote_root = LocalRoot::new();
    let mut remote_publisher = open(&remote_root);
    let remote_vault = vault(&mut remote_publisher);
    let mut source = MemoryStore::new(AdChannel::Production);
    let release = remote_publisher
        .publish_with(&mut source, &remote_vault, AdChannel::Production, 1002)
        .unwrap();
    let original = source.objects.get(&source.key()).unwrap().clone();

    let root = LocalRoot::new();
    let mut p = open(&root);
    let mut v = vault(&mut p);
    let backup = p.export_backup(b"synthetic passphrase", &v).unwrap();
    p.restore_backup(&backup, b"synthetic passphrase", &mut v)
        .unwrap();
    let before = encode(&p.state, STATE_LIMIT).unwrap();
    let key = source.key();
    let mut envelope: signed::Envelope =
        signed::strict_json(&original.bytes, ads::MAX_MANIFEST_BYTES).unwrap();
    envelope.signature[0] ^= 1;
    source.objects.get_mut(&key).unwrap().bytes =
        encode(&envelope, ads::MAX_MANIFEST_BYTES).unwrap();
    let writes = source.immutable_writes;
    let commits = source.commits;
    assert!(
        p.publish_with(&mut source, &v, AdChannel::Production, 1003)
            .is_err()
    );
    assert_eq!(encode(&p.state, STATE_LIMIT).unwrap(), before);
    assert_eq!((source.immutable_writes, source.commits), (writes, commits));

    source.objects.insert(key, original);
    source.fail_immutable = true;
    assert!(
        p.publish_with(&mut source, &v, AdChannel::Production, 1003)
            .is_err()
    );
    assert!(!p.state.recovery_required);
    assert_eq!(
        p.state.sequences[channel_index(AdChannel::Production)].highest,
        release.revision
    );
    assert!(source.immutable_writes > writes);
    assert_eq!(source.commits, commits);
    drop(p);
    let p = open(&root);
    assert!(!p.state.recovery_required);
    assert_eq!(
        p.state.sequences[channel_index(AdChannel::Production)].highest,
        release.revision
    );
}

#[test]
fn generation_creation_never_overwrites_existing_state() {
    let root = LocalRoot::new();
    let publisher = open(&root);
    let directory = root.0.join(&publisher.generation);
    let before = fs::read(directory.join("state.json")).unwrap();
    assert!(
        Publisher::write_generation(&root.0, &publisher.generation, &publisher.state, &[]).is_err()
    );
    assert_eq!(fs::read(directory.join("state.json")).unwrap(), before);
}

#[test]
fn exclusive_vault_create_preserves_existing_credentials() {
    let mut vault = MemoryVault::default();
    let original = Secrets {
        signing_seed: vec![0x41; 32],
        preview_credential: b"first".to_vec(),
        production_credential: b"production".to_vec(),
    };
    let replacement = Secrets {
        signing_seed: vec![0x42; 32],
        preview_credential: b"second".to_vec(),
        production_credential: vec![],
    };
    vault.create("generation", &original).unwrap();
    assert!(vault.create("generation", &replacement).is_err());
    let saved = vault.load("generation").unwrap();
    assert_eq!(saved.signing_seed, original.signing_seed);
    assert_eq!(saved.preview_credential, original.preview_credential);
    assert_eq!(saved.production_credential, original.production_credential);
}

#[test]
fn exclusive_vault_create_failure_preserves_existing_entries() {
    let mut vault = MemoryVault::default();
    let original = Secrets {
        signing_seed: vec![0x41; 32],
        preview_credential: vec![],
        production_credential: vec![],
    };
    vault.create("old", &original).unwrap();
    let before = vault.items.clone();
    vault.fail = true;
    assert!(vault.create("new", &original).is_err());
    assert_eq!(vault.items, before);
}

#[test]
fn exclusive_vault_create_competing_writers_have_one_winner() {
    let vault = std::sync::Arc::new(std::sync::Mutex::new(MemoryVault::default()));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|value| {
            let vault = vault.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let secret = Secrets {
                    signing_seed: vec![value; 32],
                    preview_credential: vec![],
                    production_credential: vec![],
                };
                barrier.wait();
                vault.lock().unwrap().create("same", &secret).is_ok()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|success| *success)
            .count(),
        1
    );
}
