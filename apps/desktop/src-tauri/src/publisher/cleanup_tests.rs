//! Isolated maintenance regression tests. All paths and signing keys are synthetic.
use super::{cleanup::*, *};
use ed25519_dalek::Signer;
use std::os::unix::fs::PermissionsExt;

struct CleanupRoot(PathBuf);
impl CleanupRoot {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-cleanup-{}-{}",
            std::process::id(),
            random_id().unwrap(),
        )))
    }
}
impl Drop for CleanupRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cleanup_publisher(root: &CleanupRoot) -> Publisher {
    let mut publisher = Publisher::open(
        &root.0,
        "cleanup-test",
        Some(cleanup_signer().verifying_key()),
        1000,
    )
    .unwrap();
    let mut state = publisher.state.clone();
    state.schema = 2;
    state.target = Some(format!("local:{}", "a".repeat(64)));
    publisher.persist(state).unwrap();
    publisher
}
fn cleanup_signer() -> SigningKey {
    SigningKey::from_bytes(&[0x37; 32])
}
fn cleanup_object(manifest: &AdManifest) -> protocol::Object {
    let payload = String::from_utf8(encode(manifest, ads::MAX_MANIFEST_BYTES).unwrap()).unwrap();
    let bytes = serde_json::to_vec(&signed::Envelope {
        signature: cleanup_signer()
            .sign(payload.as_bytes())
            .to_bytes()
            .to_vec(),
        payload,
    })
    .unwrap();
    protocol::Object {
        etag: signed::digest(&bytes),
        bytes,
    }
}
fn cleanup_asset(bytes: &[u8]) -> ImageAsset {
    ImageAsset {
        sha256: signed::digest(bytes),
        mime: "image/png".into(),
        bytes: bytes.len() as u32,
        width: 1,
        height: 1,
    }
}

#[test]
fn f17_retention_full_capacity_pins_currents_inside_default_128() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    for index in 0..HISTORY_LIMIT {
        let mut manifest = publisher.state.draft.clone();
        manifest.release_id = format!("history-{index}");
        manifest.content_revision = index as u64 + 1;
        manifest.channel = if index % 2 == 0 {
            AdChannel::Preview
        } else {
            AdChannel::Production
        };
        publisher.state.history.push(HistoryRecord {
            payload_hash: signed::digest(&encode(&manifest, ads::MAX_MANIFEST_BYTES).unwrap()),
            manifest,
            published_at: 1000,
        });
    }
    // No files exist for these synthetic registered assets. Index preview must
    // not depend on a successful backup or a full asset read/decode.
    publisher.state.assets = (0..ASSET_LIMIT)
        .map(|index| cleanup_asset(format!("asset-{index}").as_bytes()))
        .collect();
    let objects = [
        Some(cleanup_object(&publisher.state.history[0].manifest)),
        Some(cleanup_object(&publisher.state.history[1].manifest)),
    ];
    let before = encode(&publisher.state, STATE_LIMIT).unwrap();
    let result = retention(&publisher, &objects, CleanupOptions::default(), 1000).unwrap();
    assert_eq!(result.preview.keep_history.len(), 128);
    assert!(result.preview.keep_history.contains(&0));
    assert!(result.preview.keep_history.contains(&1));
    assert_eq!(
        &result.preview.keep_history[2..],
        &(130..256).collect::<Vec<_>>()
    );
    assert_eq!(result.preview.removed_history, 128);
    assert_eq!(result.preview.candidates.len(), 128);
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), before);
    assert!(
        retention(
            &publisher,
            &objects,
            CleanupOptions {
                retain: 1,
                release_resolution: false
            },
            1000
        )
        .is_err()
    );
    assert!(
        retention(
            &publisher,
            &objects,
            CleanupOptions {
                retain: 257,
                release_resolution: false
            },
            1000
        )
        .is_err()
    );
    let all = retention(
        &publisher,
        &objects,
        CleanupOptions {
            retain: 256,
            release_resolution: false,
        },
        1000,
    )
    .unwrap();
    assert_eq!(all.preview.removed_history, 0);
}

#[test]
fn f17_current_without_history_does_not_create_verified_history_and_expiry_is_allowed() {
    let root = CleanupRoot::new();
    let publisher = cleanup_publisher(&root);
    let current = cleanup_object(&publisher.state.draft);
    let result = retention(
        &publisher,
        &[Some(current), None],
        CleanupOptions::default(),
        publisher.state.draft.expires_at + 1,
    )
    .unwrap();
    assert!(result.preview.keep_history.is_empty());
    assert_eq!(publisher.state.history.len(), 0);
    assert_eq!(
        result.sequences[0].highest,
        publisher.state.draft.content_revision
    );
}

#[test]
fn f17_current_watermark_authentication_and_stale_preview_binding() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let object = cleanup_object(&publisher.state.draft);
    let original = retention(
        &publisher,
        &[Some(object.clone()), None],
        CleanupOptions::default(),
        1000,
    )
    .unwrap();
    publisher.state.sequences = original.sequences;
    assert!(retention(&publisher, &[None, None], CleanupOptions::default(), 1000).is_err());
    let mut changed = publisher.state.draft.clone();
    changed.release_id = "same-sequence-different-payload".into();
    assert!(
        retention(
            &publisher,
            &[Some(cleanup_object(&changed)), None],
            CleanupOptions::default(),
            1000
        )
        .is_err()
    );
    changed.content_revision += 1;
    changed.issued_at = 1001;
    assert!(
        retention(
            &publisher,
            &[Some(cleanup_object(&changed)), None],
            CleanupOptions::default(),
            1000
        )
        .is_err()
    );
    let mut bad = object.clone();
    bad.bytes[0] ^= 1;
    assert!(
        retention(
            &publisher,
            &[Some(bad), None],
            CleanupOptions::default(),
            1000
        )
        .is_err()
    );
    let baseline = retention(
        &publisher,
        &[Some(object.clone()), None],
        CleanupOptions::default(),
        1000,
    )
    .unwrap();
    let mut etag = object.clone();
    etag.etag.push('x');
    let other = retention(
        &publisher,
        &[Some(etag), None],
        CleanupOptions::default(),
        1000,
    )
    .unwrap();
    assert_ne!(baseline.preview.digest, other.preview.digest);
    publisher.state.recovery_required = !publisher.state.recovery_required;
    let changed_state = retention(
        &publisher,
        &[Some(object), None],
        CleanupOptions::default(),
        1000,
    )
    .unwrap();
    assert_ne!(baseline.preview.digest, changed_state.preview.digest);
}

#[test]
fn f17_pending_allows_read_only_preview_and_marks_execution_blocked() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.pending = Some(Pending {
        manifest: publisher.state.draft.clone(),
        wire: cleanup_object(&publisher.state.draft).bytes,
        expected_etag: None,
        phase: Phase::Prepared,
    });
    let before = encode(&publisher.state, STATE_LIMIT).unwrap();
    let preview = retention(&publisher, &[None, None], CleanupOptions::default(), 1000)
        .unwrap()
        .preview;
    assert!(preview.pending_blocks_commit);
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), before);
}

#[test]
fn f17_candidate_identity_is_bounded_no_follow_and_content_bound() {
    let root = CleanupRoot::new();
    let publisher = cleanup_publisher(&root);
    let directory = publisher.directory().join("assets");
    let bytes = b"synthetic registered content";
    let asset = cleanup_asset(bytes);
    let path = directory.join(&asset.sha256);
    assert!(inspect_candidate(&directory, &asset).unwrap().is_none());
    storage::atomic_write(&path, bytes).unwrap();
    let identity = inspect_candidate(&directory, &asset).unwrap().unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(inspect_candidate(&directory, &asset).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let alias = directory.join("not-a-candidate");
    fs::hard_link(&path, &alias).unwrap();
    assert!(inspect_candidate(&directory, &asset).is_err());
    fs::remove_file(&alias).unwrap();
    storage::atomic_write(&path, b"changed content").unwrap();
    assert!(inspect_candidate(&directory, &asset).is_err());
    storage::atomic_write(&path, bytes).unwrap();
    assert_ne!(
        inspect_candidate(&directory, &asset).unwrap().unwrap().ino,
        identity.ino
    );
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&alias, &path).unwrap();
    assert!(inspect_candidate(&directory, &asset).is_err());
    assert!(
        fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

struct CleanupCurrents([Option<protocol::Object>; 2]);
impl FixedCurrents for CleanupCurrents {
    fn target_identity(&self) -> Option<String> {
        Some(format!("local:{}", "a".repeat(64)))
    }
    fn read_currents(
        &mut self,
    ) -> std::result::Result<[Option<protocol::Object>; 2], protocol::StoreError> {
        Ok(self.0.clone())
    }
}

struct MultiCleanupCurrents {
    active: [Option<protocol::Object>; 2],
    others: BTreeMap<String, [Option<protocol::Object>; 2]>,
}
impl FixedCurrents for MultiCleanupCurrents {
    fn target_identity(&self) -> Option<String> {
        Some(format!("local:{}", "a".repeat(64)))
    }
    fn read_currents(
        &mut self,
    ) -> std::result::Result<[Option<protocol::Object>; 2], protocol::StoreError> {
        Ok(self.active.clone())
    }
    fn supports_multiple_targets(&self) -> bool {
        true
    }
    fn read_other_currents(
        &mut self,
        _targets: &[String],
    ) -> Result<BTreeMap<String, [Option<protocol::Object>; 2]>> {
        Ok(self.others.clone())
    }
}

#[test]
fn multi_target_cleanup_checks_complete_sources_and_binds_current_to_confirmation() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.schema = 3;
    let target = format!("local:{}", "b".repeat(64));
    publisher
        .state
        .other_targets
        .insert(target.clone(), TargetLedger::default());
    let pinned = cleanup_asset(b"inactive-current");
    let unused = cleanup_asset(b"unreferenced");
    publisher.state.assets = vec![pinned.clone(), unused.clone()];
    let unused_path = publisher.directory().join("assets").join(&unused.sha256);
    storage::atomic_write(&unused_path, b"unreferenced").unwrap();
    let manifest = cleanup_manifest(
        &publisher,
        pinned.clone(),
        "other-current",
        7,
        AdChannel::Preview,
    );
    let mut source = MultiCleanupCurrents {
        active: [None, None],
        others: BTreeMap::new(),
    };
    assert!(matches!(
        publisher.cleanup_preview(&mut source, CleanupOptions::default(), 1000),
        Err(Error::Invalid("cleanup_target_set_mismatch"))
    ));
    source
        .others
        .insert(target.clone(), [Some(cleanup_object(&manifest)), None]);
    let preview = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    assert_eq!(preview.candidates, vec![unused.clone()]);
    source.others.get_mut(&target).unwrap()[0]
        .as_mut()
        .unwrap()
        .etag = "changed-etag".into();
    assert!(matches!(
        publisher.cleanup_commit(
            &mut source,
            CleanupOptions::default(),
            &preview.digest,
            1000
        ),
        Err(Error::Invalid("cleanup_stale_preview"))
    ));
    let preview = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    publisher
        .cleanup_commit(
            &mut source,
            CleanupOptions::default(),
            &preview.digest,
            1000,
        )
        .unwrap();
    assert!(publisher.state.assets.contains(&pinned));
    assert!(!publisher.state.assets.contains(&unused));
    assert_eq!(
        publisher.state.other_targets[&target].sequences[0].highest,
        7
    );
    source.others.get_mut(&target).unwrap()[0] = None;
    assert!(matches!(
        publisher.gc_preview(&mut source, 1000),
        Err(Error::Invalid("cleanup_current_missing"))
    ));
    let changed = cleanup_manifest(
        &publisher,
        unused.clone(),
        "new-reference",
        8,
        AdChannel::Preview,
    );
    source.others.get_mut(&target).unwrap()[0] = Some(cleanup_object(&changed));
    assert!(matches!(
        publisher.gc_preview(&mut source, 1000),
        Err(Error::Invalid("cleanup_unregistered_reference"))
    ));
    assert!(unused_path.exists());
    source.others.get_mut(&target).unwrap()[0] = Some(cleanup_object(&manifest));
    let gc = publisher.gc_preview(&mut source, 1000).unwrap();
    let outcome = publisher.gc_commit(&mut source, &gc.digest, 1000).unwrap();
    assert!(outcome.deleted.contains(&unused.sha256));
    assert!(!unused_path.exists());
}

#[test]
fn multi_target_cleanup_rejects_inactive_rollback_conflict_and_tampering() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.schema = 3;
    let target = format!("local:{}", "b".repeat(64));
    let image = cleanup_asset(b"other-current");
    publisher.state.assets.push(image.clone());
    let current = cleanup_manifest(&publisher, image.clone(), "current", 10, AdChannel::Preview);
    let mut ledger = TargetLedger::default();
    ledger.sequences[0] = signed::Sequence {
        highest: 10,
        payload_hash: Some(signed::digest(
            &encode(&current, ads::MAX_MANIFEST_BYTES).unwrap(),
        )),
    };
    publisher.state.other_targets.insert(target.clone(), ledger);
    let mut older = current.clone();
    older.content_revision = 9;
    let mut source = MultiCleanupCurrents {
        active: [None, None],
        others: BTreeMap::from([(target.clone(), [Some(cleanup_object(&older)), None])]),
    };
    assert!(matches!(
        publisher.cleanup_preview(&mut source, CleanupOptions::default(), 1000),
        Err(Error::Invalid("old_sequence"))
    ));
    let mut conflict = current.clone();
    conflict.release_id = "different-current".into();
    source.others.get_mut(&target).unwrap()[0] = Some(cleanup_object(&conflict));
    assert!(matches!(
        publisher.cleanup_preview(&mut source, CleanupOptions::default(), 1000),
        Err(Error::Invalid("sequence_conflict"))
    ));
    let mut tampered = cleanup_object(&current);
    tampered.bytes[0] = b'!';
    source.others.get_mut(&target).unwrap()[0] = Some(tampered);
    assert!(
        publisher
            .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
            .is_err()
    );
    assert_eq!(
        publisher.state.other_targets[&target].sequences[0].highest,
        10
    );
    assert!(publisher.state.cleanup.is_none());
}

#[test]
fn local_multi_source_holds_locks_and_checks_each_real_target_identity() {
    use super::local_store::{LocalObjectStore, MultiLocalSources};
    use super::protocol::ObjectStore;
    use switcher_core::ad_runtime::local::{LocalTrust, directory};
    let root = CleanupRoot::new();
    let a_root = CleanupRoot::new();
    let b_root = CleanupRoot::new();
    let mut publisher = Publisher::open(
        &root.0,
        "ads-personal-v1",
        Some(cleanup_signer().verifying_key()),
        1000,
    )
    .unwrap();
    for path in [&a_root.0, &b_root.0] {
        storage::create_private_directory(path).unwrap();
        storage::create_private_directory(&path.join("v2")).unwrap();
    }
    let trust = LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: cleanup_signer().verifying_key().to_bytes(),
    };
    let a = LocalObjectStore::open(&a_root.0, AdChannel::Preview, &trust).unwrap();
    let mut b = LocalObjectStore::open(&b_root.0, AdChannel::Preview, &trust).unwrap();
    let a_id = ObjectStore::target_identity(&a).unwrap();
    let b_id = ObjectStore::target_identity(&b).unwrap();
    publisher.state.schema = 3;
    publisher.state.target = Some(a_id);
    publisher
        .state
        .other_targets
        .insert(b_id.clone(), TargetLedger::default());
    let pinned = cleanup_asset(b"local-other-current");
    let unused = cleanup_asset(b"local-unused");
    publisher.state.assets = vec![pinned.clone(), unused.clone()];
    let unused_path = publisher.directory().join("assets").join(&unused.sha256);
    storage::atomic_write(&unused_path, b"local-unused").unwrap();
    let current = cleanup_manifest(
        &publisher,
        pinned.clone(),
        "local-b-current",
        3,
        AdChannel::Preview,
    );
    b.commit_current(None, &cleanup_object(&current).bytes)
        .unwrap();
    let mut sources = MultiLocalSources::new(a, vec![b]).unwrap();
    assert!(LocalObjectStore::open(&a_root.0, AdChannel::Preview, &trust).is_err());
    assert!(LocalObjectStore::open(&b_root.0, AdChannel::Preview, &trust).is_err());
    let preview = publisher
        .cleanup_preview(&mut sources, CleanupOptions::default(), 1000)
        .unwrap();
    assert_eq!(preview.candidates, vec![unused.clone()]);
    assert_eq!(preview.other_currents.len(), 1);
    publisher
        .cleanup_commit(
            &mut sources,
            CleanupOptions::default(),
            &preview.digest,
            1000,
        )
        .unwrap();
    assert_eq!(publisher.state.other_targets[&b_id].sequences[0].highest, 3);
    let original = directory(&b_root.0);
    let moved = b_root.0.join("displaced-publication-tree");
    fs::rename(&original, &moved).unwrap();
    storage::create_private_directory(&original).unwrap();
    assert!(publisher.gc_preview(&mut sources, 1000).is_err());
    assert!(publisher.state.assets.contains(&pinned));
    assert!(unused_path.exists());
    fs::remove_dir(&original).unwrap();
    fs::rename(&moved, &original).unwrap();
    let gc = publisher.gc_preview(&mut sources, 1000).unwrap();
    let outcome = publisher.gc_commit(&mut sources, &gc.digest, 1000).unwrap();
    assert!(outcome.deleted.contains(&unused.sha256));
    assert!(!unused_path.exists());
    drop(sources);
    assert!(LocalObjectStore::open(&b_root.0, AdChannel::Preview, &trust).is_ok());
}
#[test]
fn migration_confirmation_preserves_old_generation_and_vault() {
    use super::security::{Secrets, Vault};
    use super::test_support::MemoryVault;
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let old_generation = publisher.generation.clone();
    let old_state = encode(&publisher.state, STATE_LIMIT).unwrap();
    let mut vault = MemoryVault::default();
    vault
        .store(
            &old_generation,
            &Secrets {
                signing_seed: vec![0x37; 32],
                preview_credential: vec![],
                production_credential: vec![],
            },
        )
        .unwrap();
    let mut currents = CleanupCurrents([None, None]);
    let preview = publisher.migration_preview(&mut currents, 1000).unwrap();
    currents.0[0] = Some(cleanup_object(&publisher.state.draft));
    assert!(
        publisher
            .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
            .is_err()
    );
    assert_eq!(publisher.generation, old_generation);
    currents.0[0] = None;
    publisher
        .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
        .unwrap();
    assert_ne!(publisher.generation, old_generation);
    assert_eq!(
        fs::read(root.0.join(&old_generation).join("state.json")).unwrap(),
        old_state
    );
    assert!(vault.items.contains_key(&old_generation));
    assert!(vault.items.contains_key(&publisher.generation));
    assert_eq!(
        publisher.state.sequences,
        [signed::Sequence::default(), signed::Sequence::default()]
    );
}

#[test]
fn migration_rejects_invalid_target_and_keeps_unknown_pending_on_failure() {
    use super::security::{Secrets, Vault};
    use super::test_support::MemoryVault;
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.schema = 1;
    publisher.state.target = None;
    publisher.state.pending = Some(Pending {
        manifest: publisher.state.draft.clone(),
        wire: cleanup_object(&publisher.state.draft).bytes,
        expected_etag: None,
        phase: Phase::CommitUnknown,
    });
    publisher.persist(publisher.state.clone()).unwrap();
    let old_generation = publisher.generation.clone();
    let old_state = encode(&publisher.state, STATE_LIMIT).unwrap();
    let mut vault = MemoryVault::default();
    vault
        .store(
            &old_generation,
            &Secrets {
                signing_seed: vec![0x37; 32],
                preview_credential: vec![],
                production_credential: vec![],
            },
        )
        .unwrap();
    let mut currents = CleanupCurrents([
        None,
        Some(protocol::Object {
            bytes: b"invalid".to_vec(),
            etag: "tag".into(),
        }),
    ]);
    assert!(publisher.migration_preview(&mut currents, 1000).is_err());
    currents.0[1] = None;
    let preview = publisher.migration_preview(&mut currents, 1000).unwrap();
    vault.fail = true;
    assert!(
        publisher
            .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
            .is_err()
    );
    assert_eq!(publisher.generation, old_generation);
    assert_eq!(
        fs::read(root.0.join(&old_generation).join("state.json")).unwrap(),
        old_state
    );
    vault.fail = false;
    publisher
        .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
        .unwrap();
    assert!(publisher.state.pending.is_none());
    assert_eq!(
        fs::read(root.0.join(&old_generation).join("state.json")).unwrap(),
        old_state
    );
    let views = serde_json::to_value(publisher.retained_views().unwrap()).unwrap();
    assert_eq!(views[0]["pending_target_unknown"], true);
    assert!(views[0]["pending_release_id"].as_str().is_some());
}

#[test]
fn migration_preserves_legacy_cleanup_and_cross_target_checkpoint() {
    use super::security::{Secrets, Vault};
    use super::test_support::MemoryVault;
    for legacy in [true, false] {
        let (root, mut publisher, mut currents, _) = staged_cleanup(1);
        let old_generation = publisher.generation.clone();
        if legacy {
            publisher.state.schema = 1;
            publisher.state.target = None;
        } else {
            publisher.state.target = Some(format!("local:{}", "b".repeat(64)));
        }
        publisher.persist(publisher.state.clone()).unwrap();
        let old_state = encode(&publisher.state, STATE_LIMIT).unwrap();
        let checkpoint = fs::read(publisher.directory().join(CHECKPOINT_FILE)).unwrap();
        let mut vault = MemoryVault::default();
        vault
            .store(
                &old_generation,
                &Secrets {
                    signing_seed: vec![0x37; 32],
                    preview_credential: vec![],
                    production_credential: vec![],
                },
            )
            .unwrap();
        let preview = publisher.migration_preview(&mut currents, 1000).unwrap();
        publisher
            .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
            .unwrap();
        assert!(publisher.state.cleanup.is_none());
        assert_eq!(
            fs::read(root.0.join(&old_generation).join("state.json")).unwrap(),
            old_state
        );
        assert_eq!(
            fs::read(root.0.join(old_generation).join(CHECKPOINT_FILE)).unwrap(),
            checkpoint
        );
    }
}

#[test]
fn migration_watermarks_are_only_from_verified_new_target() {
    use super::security::{Secrets, Vault};
    use super::test_support::MemoryVault;
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.sequences[0].highest = 70;
    publisher.state.sequences[0].payload_hash = Some("a".repeat(64));
    publisher.persist(publisher.state.clone()).unwrap();
    let mut current = publisher.state.draft.clone();
    current.channel = AdChannel::Production;
    current.content_revision = 20;
    let mut currents = CleanupCurrents([None, Some(cleanup_object(&current))]);
    let mut vault = MemoryVault::default();
    vault
        .store(
            &publisher.generation,
            &Secrets {
                signing_seed: vec![0x37; 32],
                preview_credential: vec![],
                production_credential: vec![],
            },
        )
        .unwrap();
    let preview = publisher.migration_preview(&mut currents, 1000).unwrap();
    publisher
        .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
        .unwrap();
    assert_eq!(publisher.state.sequences[0].highest, 0);
    assert_eq!(publisher.state.sequences[1].highest, 20);
    assert_eq!(publisher.state.draft.content_revision, 71);
}

#[test]
fn migration_after_backup_restore_to_another_directory_preserves_restored_generation() {
    use super::security::{Secrets, Vault};
    use super::test_support::MemoryVault;
    let old_root = CleanupRoot::new();
    let source = cleanup_publisher(&old_root);
    let mut source_vault = MemoryVault::default();
    source_vault
        .store(
            &source.generation,
            &Secrets {
                signing_seed: vec![0x37; 32],
                preview_credential: vec![],
                production_credential: vec![],
            },
        )
        .unwrap();
    let backup = source
        .export_backup(b"synthetic backup passphrase", &source_vault)
        .unwrap();
    let new_root = CleanupRoot::new();
    let mut restored = Publisher::open(
        &new_root.0,
        "cleanup-test",
        Some(cleanup_signer().verifying_key()),
        1000,
    )
    .unwrap();
    let mut vault = MemoryVault::default();
    restored
        .restore_backup(&backup, b"synthetic backup passphrase", &mut vault)
        .unwrap();
    let generation = restored.generation.clone();
    let state = encode(&restored.state, STATE_LIMIT).unwrap();
    struct OtherTarget;
    impl FixedCurrents for OtherTarget {
        fn target_identity(&self) -> Option<String> {
            Some(format!("local:{}", "b".repeat(64)))
        }
        fn read_currents(
            &mut self,
        ) -> std::result::Result<[Option<protocol::Object>; 2], protocol::StoreError> {
            Ok([None, None])
        }
    }
    let mut currents = OtherTarget;
    let preview = restored.migration_preview(&mut currents, 1000).unwrap();
    restored
        .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
        .unwrap();
    assert_eq!(
        fs::read(new_root.0.join(&generation).join("state.json")).unwrap(),
        state
    );
    assert!(vault.items.contains_key(&generation));
}

#[test]
fn migration_post_commit_fault_reports_unknown_and_restart_keeps_both_generations() {
    use super::security::{Secrets, Vault};
    use super::test_support::MemoryVault;
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let old_generation = publisher.generation.clone();
    let old_state = encode(&publisher.state, STATE_LIMIT).unwrap();
    let mut vault = MemoryVault::default();
    vault
        .store(
            &old_generation,
            &Secrets {
                signing_seed: vec![0x37; 32],
                preview_credential: vec![],
                production_credential: vec![],
            },
        )
        .unwrap();
    let mut currents = CleanupCurrents([None, None]);
    let preview = publisher.migration_preview(&mut currents, 1000).unwrap();
    super::migration::ACTIVE_AFTER_COMMIT_FAULT.with(|fault| fault.set(true));
    assert!(matches!(
        publisher.migration_commit(&mut currents, &preview.digest, 1000, &mut vault),
        Err(Error::Invalid("migration_active_outcome_unknown"))
    ));
    let new_generation = publisher.generation.clone();
    assert_ne!(new_generation, old_generation);
    assert!(
        publisher
            .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
            .is_err()
    );
    drop(publisher);
    let reopened = Publisher::open(
        &root.0,
        "cleanup-test",
        Some(cleanup_signer().verifying_key()),
        1000,
    )
    .unwrap();
    assert_eq!(reopened.generation, new_generation);
    assert_eq!(
        fs::read(root.0.join(&old_generation).join("state.json")).unwrap(),
        old_state
    );
    assert!(vault.items.contains_key(&old_generation));
    assert!(vault.items.contains_key(&new_generation));
}

#[test]
fn retained_generation_export_rejects_unexpected_files_without_mutating_state() {
    use super::security::{Secrets, Vault};
    use super::test_support::MemoryVault;
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let old_generation = publisher.generation.clone();
    let mut vault = MemoryVault::default();
    vault
        .store(
            &old_generation,
            &Secrets {
                signing_seed: vec![0x37; 32],
                preview_credential: vec![],
                production_credential: vec![],
            },
        )
        .unwrap();
    let mut currents = CleanupCurrents([None, None]);
    let preview = publisher.migration_preview(&mut currents, 1000).unwrap();
    publisher
        .migration_commit(&mut currents, &preview.digest, 1000, &mut vault)
        .unwrap();
    let active = publisher.generation.clone();
    let archive = publisher
        .export_retained_generation(&old_generation, b"synthetic backup passphrase", &vault)
        .unwrap();
    assert!(backup::open(&archive, b"wrong synthetic passphrase").is_err());
    assert!(backup::open(&archive, b"synthetic backup passphrase").is_ok());
    let destination = CleanupRoot::new();
    let target = cleanup_publisher(&destination);
    let active_before = target.generation.clone();
    let mut target_vault = MemoryVault::default();
    assert!(
        target
            .import_retained_generation(&archive, b"wrong synthetic passphrase", &mut target_vault)
            .is_err()
    );
    target
        .import_retained_generation(&archive, b"synthetic backup passphrase", &mut target_vault)
        .unwrap();
    assert_eq!(target.generation, active_before);
    assert_eq!(
        fs::read(destination.0.join(&old_generation).join("state.json")).unwrap(),
        fs::read(root.0.join(&old_generation).join("state.json")).unwrap()
    );
    assert!(target_vault.items.contains_key(&old_generation));
    assert!(
        target
            .import_retained_generation(&archive, b"synthetic backup passphrase", &mut target_vault)
            .is_err()
    );
    for fault in [1, 2] {
        let partial_root = CleanupRoot::new();
        let partial = cleanup_publisher(&partial_root);
        let active = partial.generation.clone();
        let mut partial_vault = MemoryVault::default();
        if fault == 1 {
            CREATE_CHILD_SYNC_FAULT.with(|slot| slot.set(1));
        } else {
            partial_vault.fail = true;
        }
        assert!(
            partial
                .import_retained_generation(
                    &archive,
                    b"synthetic backup passphrase",
                    &mut partial_vault
                )
                .is_err()
        );
        assert_eq!(partial.generation, active);
        let marker = format!("retain-{}-{}.json", old_generation, active);
        assert!(partial_root.0.join(&marker).is_file());
        drop(partial);
        let reopened = Publisher::open(
            &partial_root.0,
            "cleanup-test",
            Some(cleanup_signer().verifying_key()),
            1000,
        )
        .unwrap();
        assert_eq!(reopened.generation, active);
        assert!(partial_root.0.join(marker).is_file());
        assert!(
            reopened
                .import_retained_generation(
                    &archive,
                    b"synthetic backup passphrase",
                    &mut partial_vault
                )
                .is_err()
        );
    }
    storage::atomic_write(
        &root.0.join(&old_generation).join("unexpected"),
        b"unexpected",
    )
    .unwrap();
    assert!(
        publisher
            .export_retained_generation(&old_generation, b"synthetic backup passphrase", &vault)
            .is_err()
    );
    assert_eq!(publisher.generation, active);
}

#[test]
fn f17_confirmed_index_cleanup_reopens_full_asset_capacity() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.assets = (0..ASSET_LIMIT)
        .map(|index| cleanup_asset(format!("unused-{index}").as_bytes()))
        .collect();
    for index in 0..HISTORY_LIMIT {
        let mut manifest = publisher.state.draft.clone();
        manifest.release_id = format!("full-{index}");
        manifest.content_revision = index as u64 + 1;
        publisher.state.history.push(HistoryRecord {
            payload_hash: signed::digest(&encode(&manifest, ads::MAX_MANIFEST_BYTES).unwrap()),
            manifest,
            published_at: 1000,
        });
    }
    publisher.persist(publisher.state.clone()).unwrap();
    let mut currents = CleanupCurrents([None, None]);
    let preview = publisher
        .cleanup_preview(&mut currents, CleanupOptions::default(), 1000)
        .unwrap();
    publisher
        .cleanup_commit(
            &mut currents,
            CleanupOptions::default(),
            &preview.digest,
            1000,
        )
        .unwrap();
    let png = [
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2,
        0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 96, 96, 96, 0, 0, 0,
        4, 0, 1, 246, 23, 56, 85, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let imported = publisher.import_image(&png, "image/png");
    assert!(
        imported.is_ok(),
        "explicitly confirmed cleanup must release logical asset capacity: {imported:?}"
    );
    assert_eq!(publisher.state.history.len(), 128);
}

#[test]
fn f17_gc_second_confirmation_reclaims_verified_candidate() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let bytes = b"gc synthetic registered content";
    let asset = cleanup_asset(bytes);
    storage::atomic_write(
        &publisher.directory().join("assets").join(&asset.sha256),
        bytes,
    )
    .unwrap();
    publisher.state.assets.push(asset.clone());
    publisher.persist(publisher.state.clone()).unwrap();
    let mut source = CleanupCurrents([None, None]);
    let index = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    publisher
        .cleanup_commit(&mut source, CleanupOptions::default(), &index.digest, 1000)
        .unwrap();
    let path = publisher.directory().join("assets").join(&asset.sha256);
    assert!(path.exists(), "index confirmation must not delete material");
    let preview = publisher.gc_preview(&mut source, 1000).unwrap();
    assert_eq!(preview.eligible, vec![asset.sha256.clone()]);
    publisher
        .gc_commit(&mut source, &preview.digest, 1000)
        .unwrap();
    assert!(
        !path.exists(),
        "second explicit confirmation must reclaim the verified candidate"
    );
    assert!(!publisher.cleanup_pending());
}

fn staged_cleanup(count: usize) -> (CleanupRoot, Publisher, CleanupCurrents, Vec<ImageAsset>) {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let assets: Vec<_> = (0..count)
        .map(|index| {
            let bytes = format!("registered cleanup candidate {index}").into_bytes();
            let asset = cleanup_asset(&bytes);
            storage::atomic_write(
                &publisher.directory().join("assets").join(&asset.sha256),
                &bytes,
            )
            .unwrap();
            asset
        })
        .collect();
    publisher.state.assets = assets.clone();
    publisher.persist(publisher.state.clone()).unwrap();
    let mut source = CleanupCurrents([None, None]);
    let preview = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    publisher
        .cleanup_commit(
            &mut source,
            CleanupOptions::default(),
            &preview.digest,
            1000,
        )
        .unwrap();
    (root, publisher, source, assets)
}

#[test]
fn f17_index_atomic_boundaries_preserve_old_state_or_durable_batch() {
    for point in [
        "checkpoint_before",
        "checkpoint_after",
        "index_before",
        "index_after",
    ] {
        let root = CleanupRoot::new();
        let mut publisher = cleanup_publisher(&root);
        publisher.state.assets.push(cleanup_asset(
            b"missing material is not a backup prerequisite",
        ));
        publisher.persist(publisher.state.clone()).unwrap();
        let mut source = CleanupCurrents([None, None]);
        let preview = publisher
            .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
            .unwrap();
        CLEANUP_FAULT.with(|fault| fault.set(Some(point)));
        assert!(
            publisher
                .cleanup_commit(
                    &mut source,
                    CleanupOptions::default(),
                    &preview.digest,
                    1000
                )
                .is_err(),
            "{point}"
        );
        assert_eq!(
            publisher.cleanup_pending(),
            point == "index_after",
            "{point}"
        );
        let state = encode(&publisher.state, STATE_LIMIT).unwrap();
        drop(publisher);
        let mut reopened = cleanup_publisher(&root);
        assert_eq!(
            encode(&reopened.state, STATE_LIMIT).unwrap(),
            state,
            "{point}"
        );
        if !reopened.cleanup_pending() {
            let retry = reopened
                .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
                .unwrap();
            assert_eq!(
                retry.previous_checkpoint.is_some(),
                point != "checkpoint_before"
            );
            reopened
                .cleanup_commit(&mut source, CleanupOptions::default(), &retry.digest, 1000)
                .unwrap();
        }
        assert!(reopened.cleanup_pending());
    }
}

#[test]
fn f17_gc_atomic_boundaries_restart_and_partial_reclaim_are_idempotent() {
    for point in [
        "gc_start_before",
        "gc_start_after",
        "gc_rename_before",
        "gc_rename_after",
        "gc_verify_before",
        "gc_unlink_after",
        "gc_sync_assets",
        "gc_sync_quarantine",
        "gc_finish_before",
        "gc_finish_after",
    ] {
        let (root, mut publisher, mut source, assets) = staged_cleanup(2);
        let preview = publisher.gc_preview(&mut source, 1000).unwrap();
        CLEANUP_FAULT.with(|fault| fault.set(Some(point)));
        assert!(
            publisher
                .gc_commit(&mut source, &preview.digest, 1000)
                .is_err(),
            "{point}"
        );
        assert_eq!(
            publisher.cleanup_pending(),
            point != "gc_finish_after",
            "{point}"
        );
        let state = encode(&publisher.state, STATE_LIMIT).unwrap();
        drop(publisher);
        let mut reopened = cleanup_publisher(&root);
        assert_eq!(
            encode(&reopened.state, STATE_LIMIT).unwrap(),
            state,
            "{point}"
        );
        if reopened.cleanup_pending() {
            let retry = reopened.gc_preview(&mut source, 1000).unwrap();
            if point == "gc_unlink_after" {
                assert_eq!(retry.already_missing.len(), 1);
                assert_eq!(retry.eligible.len(), 1);
            }
            reopened
                .gc_commit(&mut source, &retry.digest, 1000)
                .unwrap();
        }
        assert!(!reopened.cleanup_pending(), "{point}");
        assert!(reopened.directory().join(CHECKPOINT_FILE).exists());
        for asset in assets {
            assert!(
                !reopened
                    .directory()
                    .join("assets")
                    .join(asset.sha256)
                    .exists(),
                "{point}"
            );
        }
    }
}

#[test]
fn f17_gc_four_classes_finish_without_deleting_blocked_referenced_or_unknown() {
    let (_root, mut publisher, mut source, assets) = staged_cleanup(4);
    let directory = publisher.directory().join("assets");
    fs::remove_file(directory.join(&assets[1].sha256)).unwrap();
    storage::atomic_write(&directory.join(&assets[2].sha256), b"corrupt").unwrap();
    publisher.state.assets.push(assets[3].clone());
    publisher.persist(publisher.state.clone()).unwrap();
    storage::atomic_write(&directory.join("unknown"), b"not in this batch").unwrap();
    let preview = publisher.gc_preview(&mut source, 1000).unwrap();
    assert_eq!(preview.eligible, vec![assets[0].sha256.clone()]);
    assert_eq!(preview.already_missing, vec![assets[1].sha256.clone()]);
    assert_eq!(preview.blocked, vec![assets[2].sha256.clone()]);
    assert_eq!(preview.referenced, vec![assets[3].sha256.clone()]);
    let result = publisher
        .gc_commit(&mut source, &preview.digest, 1000)
        .unwrap();
    assert!(result.batch_finished);
    assert!(!publisher.cleanup_pending());
    assert_eq!(
        fs::read(directory.join(&assets[2].sha256)).unwrap(),
        b"corrupt"
    );
    assert!(directory.join(&assets[3].sha256).exists());
    assert_eq!(
        fs::read(directory.join("unknown")).unwrap(),
        b"not in this batch"
    );
    let next = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    assert!(next.previous_checkpoint.is_some());
    assert!(!next.candidates.iter().any(|asset| asset == &assets[2]));
}

#[test]
fn f17_stale_previews_and_pending_cannot_commit() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let mut source = CleanupCurrents([None, None]);
    let preview = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    assert!(
        publisher
            .cleanup_commit(
                &mut source,
                CleanupOptions {
                    retain: 0,
                    release_resolution: false
                },
                &preview.digest,
                1000
            )
            .is_err()
    );
    storage::atomic_write(
        &publisher.directory().join(CHECKPOINT_FILE),
        &encode(&publisher.state, STATE_LIMIT).unwrap(),
    )
    .unwrap();
    assert!(
        publisher
            .cleanup_commit(
                &mut source,
                CleanupOptions::default(),
                &preview.digest,
                1000
            )
            .is_err()
    );
    publisher.state.pending = Some(Pending {
        manifest: publisher.state.draft.clone(),
        wire: cleanup_object(&publisher.state.draft).bytes,
        expected_etag: None,
        phase: Phase::Prepared,
    });
    let pending = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    assert!(pending.pending_blocks_commit);
    assert!(matches!(
        publisher.cleanup_commit(
            &mut source,
            CleanupOptions::default(),
            &pending.digest,
            1000
        ),
        Err(Error::PendingOperation)
    ));
    let (_root, mut publisher, mut source, assets) = staged_cleanup(1);
    let preview = publisher.gc_preview(&mut source, 1000).unwrap();
    fs::set_permissions(
        publisher.directory().join("assets").join(&assets[0].sha256),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(
        publisher
            .gc_commit(&mut source, &preview.digest, 1000)
            .is_err()
    );
    assert_eq!(
        publisher.state.cleanup.as_ref().unwrap().phase,
        CleanupPhase::Indexed
    );
}

#[test]
fn f17_quarantine_replacement_is_retained_and_recreated_source_is_never_unlinked() {
    let (_root, mut publisher, mut source, assets) = staged_cleanup(1);
    let original = publisher.directory().join("assets").join(&assets[0].sha256);
    let saved = publisher.directory().join("assets/retained-original");
    let swap_path = original.clone();
    let save_path = saved.clone();
    CLEANUP_HOOK.with(|slot| {
        *slot.borrow_mut() = Some((
            "gc_rename_before",
            Box::new(move || {
                fs::rename(&swap_path, &save_path).unwrap();
                storage::atomic_write(&swap_path, b"unexpected replacement").unwrap();
            }),
        ))
    });
    let preview = publisher.gc_preview(&mut source, 1000).unwrap();
    assert!(
        publisher
            .gc_commit(&mut source, &preview.digest, 1000)
            .is_err()
    );
    assert!(publisher.cleanup_pending());
    let batch = publisher.state.cleanup.as_ref().unwrap();
    let quarantine = publisher
        .directory()
        .join("cleanup-gc")
        .join(format!("{}-{}", batch.id, assets[0].sha256));
    assert_eq!(fs::read(&quarantine).unwrap(), b"unexpected replacement");
    assert!(saved.exists());
    let blocked = publisher.gc_preview(&mut source, 1000).unwrap();
    assert_eq!(blocked.blocked, vec![assets[0].sha256.clone()]);
    publisher
        .gc_commit(&mut source, &blocked.digest, 1000)
        .unwrap();
    assert_eq!(fs::read(&quarantine).unwrap(), b"unexpected replacement");
    let (_root, mut publisher, mut source, assets) = staged_cleanup(1);
    let original = publisher.directory().join("assets").join(&assets[0].sha256);
    let replacement = original.clone();
    CLEANUP_HOOK.with(|slot| {
        *slot.borrow_mut() = Some((
            "gc_rename_after",
            Box::new(move || {
                storage::atomic_write(&replacement, b"new source must survive").unwrap();
            }),
        ))
    });
    let preview = publisher.gc_preview(&mut source, 1000).unwrap();
    publisher
        .gc_commit(&mut source, &preview.digest, 1000)
        .unwrap();
    assert_eq!(fs::read(original).unwrap(), b"new source must survive");
}

fn cleanup_campaign(asset: ImageAsset) -> switcher_domain::ads::Campaign {
    switcher_domain::ads::Campaign {
        campaign_id: "synthetic".into(),
        merchant_id: "synthetic".into(),
        merchant_name: "Synthetic".into(),
        title: "Synthetic".into(),
        description: "Synthetic".into(),
        image: asset,
        button_text: "Open".into(),
        target_url: "https://example.com/".into(),
        starts_at: 1000,
        ends_at: 1100,
        enabled: true,
    }
}
fn cleanup_manifest(
    publisher: &Publisher,
    asset: ImageAsset,
    release: &str,
    revision: u64,
    channel: AdChannel,
) -> AdManifest {
    let mut manifest = publisher.state.draft.clone();
    manifest.release_id = release.into();
    manifest.content_revision = revision;
    manifest.channel = channel;
    manifest.slots[0].campaigns = vec![cleanup_campaign(asset)];
    manifest
}

#[test]
fn multi_target_retention_protects_inactive_history_and_pending() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let assets: Vec<_> = (0..3)
        .map(|i| cleanup_asset(format!("inactive-target-{i}").as_bytes()))
        .collect();
    publisher.state.assets = assets.clone();
    publisher.state.schema = 3;
    let history = cleanup_manifest(
        &publisher,
        assets[0].clone(),
        "inactive-history",
        3,
        AdChannel::Preview,
    );
    let pending = cleanup_manifest(
        &publisher,
        assets[1].clone(),
        "inactive-pending",
        4,
        AdChannel::Preview,
    );
    let ledger = TargetLedger {
        history: vec![HistoryRecord {
            payload_hash: signed::digest(&encode(&history, ads::MAX_MANIFEST_BYTES).unwrap()),
            manifest: history,
            published_at: 1000,
        }],
        pending: Some(Pending {
            wire: cleanup_object(&pending).bytes,
            manifest: pending,
            expected_etag: None,
            phase: Phase::Prepared,
        }),
        ..TargetLedger::default()
    };
    publisher
        .state
        .other_targets
        .insert(format!("local:{}", "b".repeat(64)), ledger);
    let preview = retention(&publisher, &[None, None], CleanupOptions::default(), 1000)
        .unwrap()
        .preview;
    assert_eq!(preview.candidates, vec![assets[2].clone()]);
    assert!(preview.pending_blocks_commit);
}

#[test]
fn multi_target_retention_preserves_inactive_resolution_and_rejects_missing_asset() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let assets: Vec<_> = (0..3)
        .map(|i| cleanup_asset(format!("inactive-resolution-{i}").as_bytes()))
        .collect();
    publisher.state.assets = assets.clone();
    publisher.state.schema = 3;
    let previous = cleanup_manifest(
        &publisher,
        assets[0].clone(),
        "inactive-before",
        9,
        AdChannel::Preview,
    );
    let current = cleanup_manifest(
        &publisher,
        assets[1].clone(),
        "inactive-current",
        10,
        AdChannel::Preview,
    );
    let watermark = signed::Sequence {
        highest: 10,
        payload_hash: Some(signed::digest(
            &encode(&current, ads::MAX_MANIFEST_BYTES).unwrap(),
        )),
    };
    let mut ledger = TargetLedger::default();
    ledger.sequences[0] = watermark.clone();
    ledger.last_resolution = Some(Resolution {
        pending: Pending {
            wire: cleanup_object(&previous).bytes,
            manifest: previous,
            expected_etag: None,
            phase: Phase::CommitUnknown,
        },
        current_wire: cleanup_object(&current).bytes,
        current_etag: "inactive-current".into(),
        kind: ResolutionKind::Superseded,
        resolved_at: 1000,
        watermark,
        unavailable_assets: vec![],
    });
    publisher
        .state
        .other_targets
        .insert(format!("local:{}", "b".repeat(64)), ledger);
    let options = CleanupOptions {
        retain: 0,
        release_resolution: true,
    };
    let preview = retention(&publisher, &[None, None], options.clone(), 1000)
        .unwrap()
        .preview;
    assert_eq!(preview.candidates, vec![assets[2].clone()]);
    publisher.state.assets.retain(|asset| asset != &assets[0]);
    assert!(matches!(
        retention(&publisher, &[None, None], options, 1000),
        Err(Error::Invalid("cleanup_unregistered_reference"))
    ));
}

#[test]
fn f17_reference_union_resolution_release_and_checkpoint_evidence() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let assets: Vec<_> = (0..8)
        .map(|i| cleanup_asset(format!("protected-{i}").as_bytes()))
        .collect();
    publisher.state.assets = assets.clone();
    publisher.state.draft.slots[0].campaigns = vec![cleanup_campaign(assets[0].clone())];
    let pending = cleanup_manifest(
        &publisher,
        assets[1].clone(),
        "pending",
        99,
        AdChannel::Preview,
    );
    publisher.state.pending = Some(Pending {
        wire: cleanup_object(&pending).bytes,
        manifest: pending,
        expected_etag: None,
        phase: Phase::Prepared,
    });
    let history = cleanup_manifest(
        &publisher,
        assets[2].clone(),
        "history",
        8,
        AdChannel::Preview,
    );
    publisher.state.history.push(HistoryRecord {
        payload_hash: signed::digest(&encode(&history, ads::MAX_MANIFEST_BYTES).unwrap()),
        manifest: history,
        published_at: 1000,
    });
    let previous = cleanup_manifest(
        &publisher,
        assets[5].clone(),
        "resolution-pending",
        9,
        AdChannel::Preview,
    );
    let resolution_current = cleanup_manifest(
        &publisher,
        assets[6].clone(),
        "resolution-current",
        10,
        AdChannel::Preview,
    );
    let watermark = signed::Sequence {
        highest: 10,
        payload_hash: Some(signed::digest(
            &encode(&resolution_current, ads::MAX_MANIFEST_BYTES).unwrap(),
        )),
    };
    publisher.state.sequences[0] = watermark.clone();
    publisher.state.last_resolution = Some(Resolution {
        pending: Pending {
            wire: cleanup_object(&previous).bytes,
            manifest: previous,
            expected_etag: None,
            phase: Phase::CommitUnknown,
        },
        current_wire: cleanup_object(&resolution_current).bytes,
        current_etag: "resolved".into(),
        kind: ResolutionKind::Superseded,
        resolved_at: 1000,
        watermark,
        unavailable_assets: vec![],
    });
    let preview_current = cleanup_manifest(
        &publisher,
        assets[3].clone(),
        "preview-current",
        20,
        AdChannel::Preview,
    );
    let production_current = cleanup_manifest(
        &publisher,
        assets[4].clone(),
        "production-current",
        30,
        AdChannel::Production,
    );
    let mut source = CleanupCurrents([
        Some(cleanup_object(&preview_current)),
        Some(cleanup_object(&production_current)),
    ]);
    let normal = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    assert_eq!(normal.candidates, vec![assets[7].clone()]);
    assert_eq!(normal.protected_assets.len(), 7);
    publisher.state.pending = None;
    let options = CleanupOptions {
        retain: 128,
        release_resolution: true,
    };
    let release = publisher
        .cleanup_preview(&mut source, options.clone(), 1000)
        .unwrap();
    assert!(release.candidates.contains(&assets[5]) && release.candidates.contains(&assets[6]));
    publisher
        .cleanup_commit(&mut source, options, &release.digest, 1000)
        .unwrap();
    assert!(publisher.resolution_read().is_none());
    assert_eq!(publisher.state.sequences[0].highest, 20);
    assert_eq!(publisher.state.sequences[1].highest, 30);
    let checkpoint: State = signed::strict_json(
        &read_private(&publisher.directory().join(CHECKPOINT_FILE), STATE_LIMIT).unwrap(),
        STATE_LIMIT,
    )
    .unwrap();
    assert!(checkpoint.last_resolution.is_some());
    let gc = publisher.gc_preview(&mut source, 1000).unwrap();
    assert_eq!(gc.already_missing.len(), 4); // Clearing pending also releases assets[1].
}

#[derive(Default)]
struct CleanupVault(std::collections::HashMap<String, Vec<u8>>);
impl Vault for CleanupVault {
    fn create(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        use std::collections::hash_map::Entry;
        match self.0.entry(generation.into()) {
            Entry::Vacant(entry) => {
                entry.insert(encode(secrets, 32768)?);
                Ok(())
            }
            Entry::Occupied(_) => Err(Error::SecretUnavailable),
        }
    }
    fn load(&self, generation: &str) -> Result<Secrets> {
        signed::strict_json(
            self.0.get(generation).ok_or(Error::SecretUnavailable)?,
            32768,
        )
        .map_err(Error::Invalid)
    }
    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        self.0.insert(generation.into(), encode(secrets, 32768)?);
        Ok(())
    }
}
fn cleanup_vault(publisher: &mut Publisher) -> CleanupVault {
    let mut vault = CleanupVault::default();
    publisher
        .credentials_save(
            &mut vault,
            &Secrets {
                signing_seed: vec![0x37; 32],
                preview_credential: b"synthetic".to_vec(),
                production_credential: b"synthetic".to_vec(),
            },
        )
        .unwrap();
    vault
}
#[test]
fn f17_backup_portability_and_active_target_restore_refusal() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let mut vault = cleanup_vault(&mut publisher);
    let mut source = CleanupCurrents([None, None]);
    let preview = publisher
        .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
        .unwrap();
    publisher
        .cleanup_commit(
            &mut source,
            CleanupOptions::default(),
            &preview.digest,
            1000,
        )
        .unwrap();
    let backup = publisher
        .export_backup(b"synthetic-backup-passphrase", &vault)
        .unwrap();
    let plaintext = backup::open(&backup, b"synthetic-backup-passphrase").unwrap();
    let mut snapshot: Snapshot = signed::strict_json(&plaintext, backup::LIMIT).unwrap();
    assert!(snapshot.state.cleanup.is_none());
    let before = encode(&publisher.state, STATE_LIMIT).unwrap();
    assert!(
        publisher
            .restore_backup(&backup, b"synthetic-backup-passphrase", &mut vault)
            .is_err()
    );
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), before);
    let target_root = CleanupRoot::new();
    let mut target = cleanup_publisher(&target_root);
    let mut target_vault = CleanupVault::default();
    target
        .restore_backup(&backup, b"synthetic-backup-passphrase", &mut target_vault)
        .unwrap();
    assert!(!target.cleanup_pending());
    // A decrypted/authenticated imported local-only reference is stripped before
    // complete validation, never transferred to the new generation.
    snapshot.state.cleanup = publisher.state.cleanup.clone();
    let imported = backup::seal(
        &encode(&snapshot, backup::LIMIT - 76).unwrap(),
        b"synthetic-backup-passphrase",
    )
    .unwrap();
    target
        .restore_backup(&imported, b"synthetic-backup-passphrase", &mut target_vault)
        .unwrap();
    assert!(!target.cleanup_pending());
}

#[test]
fn f17_local_store_reads_two_currents_under_one_lock_and_batch_survives_save_publish() {
    use switcher_core::ad_runtime::local::{LocalTrust, directory};
    let root = CleanupRoot::new();
    storage::create_private_directory(&root.0).unwrap();
    let publisher_root = root.0.join("publisher");
    let mut publisher = Publisher::open(
        &publisher_root,
        "ads-personal-v1",
        Some(cleanup_signer().verifying_key()),
        1000,
    )
    .unwrap();
    let vault = cleanup_vault(&mut publisher);
    let trust = LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: cleanup_signer().verifying_key().to_bytes(),
    };
    storage::create_private_directory(&root.0.join("v2")).unwrap();
    let mut store =
        local_store::LocalObjectStore::open(&root.0, AdChannel::Preview, &trust).unwrap();
    assert!(local_store::LocalObjectStore::open(&root.0, AdChannel::Production, &trust).is_err());
    let preview = publisher
        .cleanup_preview(&mut store, CleanupOptions::default(), 1000)
        .unwrap();
    let commit = publisher
        .cleanup_commit(&mut store, CleanupOptions::default(), &preview.digest, 1000)
        .unwrap();
    publisher.draft_save(publisher.draft_read(), 1001).unwrap();
    publisher
        .publish_with(&mut store, &vault, AdChannel::Preview, 1001)
        .unwrap();
    assert_eq!(
        publisher.state.cleanup.as_ref().unwrap().id,
        commit.batch_id
    );
    let mut production = publisher.state.draft.clone();
    production.channel = AdChannel::Production;
    storage::atomic_write(
        &directory(&root.0).join("production/current.json"),
        &cleanup_object(&production).bytes,
    )
    .unwrap();
    let both = store.read_currents().unwrap();
    assert!(both[0].is_some() && both[1].is_some());
    assert_eq!(store.channel(), AdChannel::Preview);
}

#[test]
fn f17_cleanup_state_rename_errors_reconcile_memory_and_restart() {
    for stage in ["index", "start", "finish"] {
        for code in [1, 2] {
            let root = CleanupRoot::new();
            let mut publisher = cleanup_publisher(&root);
            let mut source = CleanupCurrents([None, None]);
            if stage != "index" {
                let p = publisher
                    .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
                    .unwrap();
                publisher
                    .cleanup_commit(&mut source, CleanupOptions::default(), &p.digest, 1000)
                    .unwrap();
            }
            CLEANUP_WRITE_FAULT.with(|slot| slot.set(Some((stage, code))));
            let result = if stage == "index" {
                let p = publisher
                    .cleanup_preview(&mut source, CleanupOptions::default(), 1000)
                    .unwrap();
                publisher
                    .cleanup_commit(&mut source, CleanupOptions::default(), &p.digest, 1000)
                    .map(|_| ())
            } else {
                let p = publisher.gc_preview(&mut source, 1000).unwrap();
                publisher
                    .gc_commit(&mut source, &p.digest, 1000)
                    .map(|_| ())
            };
            assert!(result.is_err(), "{stage}/{code}");
            let active = match stage {
                "index" => code == 2,
                "finish" => code == 1,
                _ => true,
            };
            assert_eq!(publisher.cleanup_pending(), active, "{stage}/{code}");
            let bytes = encode(&publisher.state, STATE_LIMIT).unwrap();
            drop(publisher);
            let reopened = cleanup_publisher(&root);
            assert_eq!(
                encode(&reopened.state, STATE_LIMIT).unwrap(),
                bytes,
                "{stage}/{code}"
            );
        }
    }
}

#[test]
fn f17_restart_with_both_paths_and_unexpected_io_preserves_batch() {
    let (root, mut publisher, mut source, assets) = staged_cleanup(1);
    let original = publisher.directory().join("assets").join(&assets[0].sha256);
    let p = publisher.gc_preview(&mut source, 1000).unwrap();
    CLEANUP_FAULT.with(|slot| slot.set(Some("gc_rename_after")));
    assert!(publisher.gc_commit(&mut source, &p.digest, 1000).is_err());
    storage::atomic_write(&original, b"recreated source survives restart").unwrap();
    drop(publisher);
    let mut publisher = cleanup_publisher(&root);
    CLEANUP_FAULT.with(|slot| slot.set(Some("gc_inspect_io")));
    assert!(publisher.gc_preview(&mut source, 1000).is_err());
    assert!(publisher.cleanup_pending());
    let retry = publisher.gc_preview(&mut source, 1000).unwrap();
    assert_eq!(retry.eligible, vec![assets[0].sha256.clone()]);
    publisher
        .gc_commit(&mut source, &retry.digest, 1000)
        .unwrap();
    assert_eq!(
        fs::read(original).unwrap(),
        b"recreated source survives restart"
    );
}

#[test]
fn f17_missing_only_restart_must_sync_both_directories_before_finishing() {
    for point in ["gc_finish_sync_assets", "gc_finish_sync_quarantine"] {
        let (root, mut publisher, mut source, _) = staged_cleanup(1);
        let p = publisher.gc_preview(&mut source, 1000).unwrap();
        CLEANUP_FAULT.with(|slot| slot.set(Some("gc_unlink_after")));
        assert!(publisher.gc_commit(&mut source, &p.digest, 1000).is_err());
        drop(publisher);
        let mut publisher = cleanup_publisher(&root);
        let p = publisher.gc_preview(&mut source, 1000).unwrap();
        assert_eq!(p.already_missing.len(), 1);
        assert!(p.eligible.is_empty());
        CLEANUP_FAULT.with(|slot| slot.set(Some(point)));
        assert!(
            publisher.gc_commit(&mut source, &p.digest, 1000).is_err(),
            "{point}: missing-only completion skipped required directory sync"
        );
        assert!(publisher.cleanup_pending());
        drop(publisher);
        let mut publisher = cleanup_publisher(&root);
        let p = publisher.gc_preview(&mut source, 1000).unwrap();
        publisher.gc_commit(&mut source, &p.digest, 1000).unwrap();
        assert!(!publisher.cleanup_pending());
    }
}

// Capacity stress uses an existing serialized Sequence hash field as synthetic
// padding. It is not a claim that ordinary authored campaigns reach 16 MiB.
fn f17_capacity_state(publisher: &Publisher, remaining: usize) -> State {
    let mut state = publisher.state.clone();
    state.sequences[0].payload_hash = Some(String::new());
    let base = encode(&state, STATE_LIMIT).unwrap().len();
    state.sequences[0].payload_hash = Some("a".repeat(STATE_LIMIT - remaining - base));
    assert_eq!(
        encode(&state, STATE_LIMIT).unwrap().len(),
        STATE_LIMIT - remaining
    );
    publisher.validate_state(&state).unwrap();
    state
}
#[test]
fn f17_gc_identity_capacity_is_reserved_before_checkpoint_and_ordinary_persist() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.assets = (0..ASSET_LIMIT)
        .map(|i| cleanup_asset(format!("capacity-{i}").as_bytes()))
        .collect();
    publisher.state = f17_capacity_state(&publisher, 4096);
    publisher.persist(publisher.state.clone()).unwrap();
    let mut source = CleanupCurrents([None, None]);
    let p = publisher
        .cleanup_preview(
            &mut source,
            CleanupOptions {
                retain: 256,
                release_resolution: false,
            },
            1000,
        )
        .unwrap();
    let result = publisher.cleanup_commit(&mut source, p.options.clone(), &p.digest, 1000);
    assert!(
        result.is_err(),
        "first confirmation admitted a batch without room for GC identities"
    );
    assert!(!publisher.cleanup_pending());
    assert!(!publisher.directory().join(CHECKPOINT_FILE).exists());
    let (_root, mut publisher, _, _) = staged_cleanup(128);
    let before = encode(&publisher.state, STATE_LIMIT).unwrap();
    let enlarged = f17_capacity_state(&publisher, 4096);
    assert!(
        publisher.persist(enlarged).is_err(),
        "ordinary save consumed GC identity reserve"
    );
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), before);
    let mut source = CleanupCurrents([None, None]);
    let p = publisher.gc_preview(&mut source, 1000).unwrap();
    CLEANUP_FAULT.with(|slot| slot.set(Some("gc_start_after")));
    assert!(publisher.gc_commit(&mut source, &p.digest, 1000).is_err());
    assert_eq!(
        publisher.state.cleanup.as_ref().unwrap().phase,
        CleanupPhase::GcStarted
    );
    let before = encode(&publisher.state, STATE_LIMIT).unwrap();
    let enlarged = f17_capacity_state(&publisher, 1);
    assert!(
        publisher.persist(enlarged).is_err(),
        "started batch lost its re-preview identity reserve"
    );
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), before);
}

#[test]
fn f17_pending_gc_commit_preserves_files_and_index() {
    let (root, mut publisher, mut source, assets) = staged_cleanup(1);
    publisher.state.pending = Some(Pending {
        manifest: publisher.state.draft.clone(),
        wire: cleanup_object(&publisher.state.draft).bytes,
        expected_etag: None,
        phase: Phase::Prepared,
    });
    publisher.persist(publisher.state.clone()).unwrap();
    let preview = publisher.gc_preview(&mut source, 1000).unwrap();
    assert!(preview.pending_blocks_commit);
    assert_eq!(preview.eligible, vec![assets[0].sha256.clone()]);
    let before_state = encode(&publisher.state, STATE_LIMIT).unwrap();
    let before_files = super::tests::local_tree(&root.0);
    let directory = publisher.directory().join("assets");
    let before_identity = inspect_candidate(&directory, &assets[0]).unwrap().unwrap();
    assert!(matches!(
        publisher.gc_commit(&mut source, &preview.digest, 1000),
        Err(Error::PendingOperation)
    ));
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), before_state);
    assert_eq!(super::tests::local_tree(&root.0), before_files);
    assert_eq!(
        inspect_candidate(&directory, &assets[0]).unwrap().unwrap(),
        before_identity
    );
    assert!(!publisher.directory().join("cleanup-gc").exists());
    drop(publisher);
    let reopened = cleanup_publisher(&root);
    assert_eq!(encode(&reopened.state, STATE_LIMIT).unwrap(), before_state);
    assert!(
        reopened.state.assets.is_empty(),
        "index cleanup remains committed"
    );
    assert_eq!(
        reopened.state.cleanup.as_ref().unwrap().phase,
        CleanupPhase::Indexed
    );
}
#[test]
fn archive_directory_handle_detects_replacement_and_symlink() {
    use std::os::unix::fs::symlink;
    let root = CleanupRoot::new();
    let _publisher = cleanup_publisher(&root);
    let path = root.0.join("archive-source");
    storage::create_private_directory(&path).unwrap();
    storage::atomic_write(&path.join("state.json"), b"synthetic").unwrap();
    let handle = cleanup::PrivateDirectory::open(&path).unwrap();
    assert_eq!(handle.names(4).unwrap(), vec!["state.json"]);
    storage::create_private_directory(&path.join("assets")).unwrap();
    let child = handle.child("assets").unwrap();
    symlink(path.join("assets"), path.join("alias")).unwrap();
    assert!(handle.child("alias").is_err());
    fs::remove_file(path.join("alias")).unwrap();
    fs::rename(path.join("assets"), path.join("assets-old")).unwrap();
    storage::create_private_directory(&path.join("assets")).unwrap();
    assert!(child.names(4).is_err());
    assert_eq!(
        handle.read_archive_file("state.json", 32).unwrap(),
        b"synthetic"
    );
    assert!(handle.read_archive_file("state.json", 2).is_err());
    symlink(path.join("state.json"), path.join("link.json")).unwrap();
    assert!(handle.read_archive_file("link.json", 32).is_err());
    fs::hard_link(path.join("state.json"), path.join("hard.json")).unwrap();
    assert!(handle.read_archive_file("state.json", 32).is_err());
    assert!(handle.read_archive_file("hard.json", 32).is_err());
    fs::remove_file(path.join("hard.json")).unwrap();
    fs::remove_file(path.join("link.json")).unwrap();
    let fifo = std::ffi::CString::new(path.join("fifo").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(handle.read_archive_file("fifo", 32).is_err());
    assert!(handle.names(1).is_err());
    fs::remove_file(path.join("fifo")).unwrap();
    let moved = root.0.join("archive-old");
    fs::rename(&path, &moved).unwrap();
    storage::create_private_directory(&path).unwrap();
    assert!(handle.names(4).is_err());
    fs::remove_dir(&path).unwrap();
    symlink(&moved, &path).unwrap();
    assert!(cleanup::PrivateDirectory::open(&path).is_err());
    assert!(handle.names(4).is_err());
}

#[test]
fn archive_directory_enumeration_rejects_concurrent_insert_and_delete() {
    use std::sync::{Arc, Barrier};
    for insert in [true, false] {
        let root = CleanupRoot::new();
        let _publisher = cleanup_publisher(&root);
        let path = root.0.join("archive-source");
        storage::create_private_directory(&path).unwrap();
        let item = path.join("item");
        if !insert {
            storage::atomic_write(&item, b"synthetic").unwrap();
        }
        let handle = cleanup::PrivateDirectory::open(&path).unwrap();
        let begin = Arc::new(Barrier::new(2));
        let done = Arc::new(Barrier::new(2));
        let worker_begin = begin.clone();
        let worker_done = done.clone();
        let worker = std::thread::spawn(move || {
            worker_begin.wait();
            if insert {
                storage::atomic_write(&item, b"synthetic").unwrap();
            } else {
                fs::remove_file(&item).unwrap();
            }
            worker_done.wait();
        });
        cleanup::ARCHIVE_ENUM_HOOK.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || {
                begin.wait();
                done.wait();
            }));
        });
        assert!(handle.names(8).is_err());
        worker.join().unwrap();
    }
}

#[test]
fn fixed_parent_child_creation_is_exclusive_and_private() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = CleanupRoot::new();
    let _publisher = cleanup_publisher(&root);
    let path = root.0.join("archive-create");
    storage::create_private_directory(&path).unwrap();
    let parent = cleanup::PrivateDirectory::open(&path).unwrap();
    let child = parent.create_child("new-generation").unwrap();
    assert!(child.names(1).unwrap().is_empty());
    assert_eq!(
        fs::metadata(path.join("new-generation")).unwrap().mode() & 0o7777,
        0o700
    );
    storage::atomic_write(&path.join("new-generation/keep"), b"keep").unwrap();
    assert!(parent.create_child("new-generation").is_err());
    assert_eq!(fs::read(path.join("new-generation/keep")).unwrap(), b"keep");
    fs::write(path.join("file"), b"keep").unwrap();
    assert!(parent.create_child("file").is_err());
    symlink(path.join("new-generation"), path.join("link")).unwrap();
    assert!(parent.create_child("link").is_err());
    assert!(parent.create_child("../escape").is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(parent.create_child("denied").is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!path.join("denied").exists());
}

#[test]
fn fixed_parent_child_creation_rejects_parent_swap_and_sync_failure() {
    let root = CleanupRoot::new();
    let _publisher = cleanup_publisher(&root);
    let path = root.0.join("archive-create");
    let moved = root.0.join("archive-moved");
    storage::create_private_directory(&path).unwrap();
    let parent = cleanup::PrivateDirectory::open(&path).unwrap();
    let hook_path = path.clone();
    let hook_moved = moved.clone();
    cleanup::CREATE_CHILD_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            fs::rename(&hook_path, &hook_moved).unwrap();
            storage::create_private_directory(&hook_path).unwrap();
        }))
    });
    assert!(parent.create_child("new").is_err());
    assert!(moved.join("new").is_dir());
    assert!(!path.join("new").exists());
    let replacement = cleanup::PrivateDirectory::open(&path).unwrap();
    for (fault_stage, name) in [(1, "uncertain-child"), (2, "uncertain-parent")] {
        cleanup::CREATE_CHILD_SYNC_FAULT.with(|fault| fault.set(fault_stage));
        assert!(replacement.create_child(name).is_err());
        assert!(path.join(name).is_dir());
        assert!(replacement.create_child(name).is_err());
    }
}

#[test]
fn fixed_parent_child_creation_rejects_child_replacement_before_open() {
    let root = CleanupRoot::new();
    let _publisher = cleanup_publisher(&root);
    let path = root.0.join("archive-create");
    storage::create_private_directory(&path).unwrap();
    let parent = cleanup::PrivateDirectory::open(&path).unwrap();
    let hook_path = path.clone();
    cleanup::CREATE_CHILD_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            fs::rename(hook_path.join("new"), hook_path.join("created-original")).unwrap();
            storage::create_private_directory(&hook_path.join("new")).unwrap();
        }))
    });
    assert!(parent.create_child("new").is_err());
    assert!(path.join("created-original").is_dir());
    assert!(path.join("new").is_dir());
}

#[test]
fn fixed_child_file_creation_is_exclusive_and_verifies_content() {
    use std::os::unix::fs::symlink;
    let root = CleanupRoot::new();
    let _publisher = cleanup_publisher(&root);
    let path = root.0.join("file-parent");
    storage::create_private_directory(&path).unwrap();
    let parent = cleanup::PrivateDirectory::open(&path).unwrap();
    parent
        .create_verified_file("state.json", b"first", 32)
        .unwrap();
    assert_eq!(
        parent.read_archive_file("state.json", 32).unwrap(),
        b"first"
    );
    assert!(
        parent
            .create_verified_file("state.json", b"second", 32)
            .is_err()
    );
    assert_eq!(fs::read(path.join("state.json")).unwrap(), b"first");
    symlink(path.join("state.json"), path.join("link")).unwrap();
    assert!(parent.create_verified_file("link", b"changed", 32).is_err());
    assert!(parent.create_verified_file("../escape", b"x", 32).is_err());
    assert!(parent.create_verified_file("oversize", b"123", 2).is_err());
    assert!(!path.join("oversize").exists());
}

#[test]
fn fixed_child_file_creation_detects_replacement_and_sync_errors() {
    for scenario in 0..4 {
        let root = CleanupRoot::new();
        let _publisher = cleanup_publisher(&root);
        let path = root.0.join("file-parent");
        storage::create_private_directory(&path).unwrap();
        let parent = cleanup::PrivateDirectory::open(&path).unwrap();
        if scenario < 2 {
            let hook_path = path.clone();
            cleanup::CREATE_FILE_HOOK.with(|slot| {
                *slot.borrow_mut() = Some(Box::new(move || {
                    if scenario == 0 {
                        fs::rename(hook_path.join("state"), hook_path.join("original")).unwrap();
                        fs::write(hook_path.join("state"), b"decoy").unwrap();
                        fs::set_permissions(
                            hook_path.join("state"),
                            fs::Permissions::from_mode(0o600),
                        )
                        .unwrap();
                    } else {
                        fs::rename(&hook_path, hook_path.with_file_name("moved")).unwrap();
                        storage::create_private_directory(&hook_path).unwrap();
                    }
                }))
            });
        } else {
            cleanup::CREATE_FILE_SYNC_FAULT.with(|fault| fault.set((scenario - 1) as u8));
        }
        assert!(
            parent
                .create_verified_file("state", b"original", 32)
                .is_err()
        );
        match scenario {
            0 => assert_eq!(fs::read(path.join("original")).unwrap(), b"original"),
            1 => assert_eq!(fs::read(root.0.join("moved/state")).unwrap(), b"original"),
            _ => assert_eq!(fs::read(path.join("state")).unwrap(), b"original"),
        }
    }
}

#[test]
fn fixed_child_file_creation_has_one_winner_under_concurrency() {
    use std::sync::{Arc, Barrier};
    let root = CleanupRoot::new();
    let _publisher = cleanup_publisher(&root);
    let path = root.0.join("file-parent");
    storage::create_private_directory(&path).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|i| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let parent = cleanup::PrivateDirectory::open(&path).unwrap();
                barrier.wait();
                parent.create_verified_file("state", &[i], 1).is_ok()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|ok| *ok)
            .count(),
        1
    );
    assert_eq!(fs::read(path.join("state")).unwrap().len(), 1);
}
