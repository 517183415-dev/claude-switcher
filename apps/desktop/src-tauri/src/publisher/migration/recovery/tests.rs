use super::*;
use crate::publisher::test_support::MemoryVault;
use switcher_core::ad_runtime::local::LocalTrust;

const NOW: u64 = 1_800_000_000;
const PASSWORD: &[u8] = b"synthetic-recovery-passphrase";
struct Fixture {
    app: PathBuf,
    publisher: Option<Publisher>,
    vault: MemoryVault,
}
impl Fixture {
    fn new() -> Self {
        let app = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("recovery-{}", random_id().unwrap()));
        storage::create_private_directory(&app).unwrap();
        storage::create_private_directory(&app.join("v2")).unwrap();
        let mut publisher =
            Publisher::open(&app.join("publisher"), "ads-personal-v1", None, NOW).unwrap();
        let mut vault = MemoryVault::default();
        publisher
            .credentials_save(
                &mut vault,
                &Secrets {
                    signing_seed: vec![37; 32],
                    preview_credential: b"synthetic-preview".to_vec(),
                    production_credential: b"synthetic-production".to_vec(),
                },
            )
            .unwrap();
        Self {
            app,
            publisher: Some(publisher),
            vault,
        }
    }
    fn local(&self) -> LocalObjectStore {
        LocalObjectStore::open(
            &self.app,
            AdChannel::Preview,
            &LocalTrust {
                schema: 1,
                key_id: "ads-personal-v1".into(),
                public_key: SigningKey::from_bytes(&[37; 32]).verifying_key().to_bytes(),
            },
        )
        .unwrap()
    }
    fn prepare(&mut self) -> Result<PreparedRecovery> {
        let source = self.local();
        PreparedRecovery::prepare(
            self.publisher.take().unwrap(),
            source,
            Zeroizing::new(PASSWORD.to_vec()),
            &self.vault,
            NOW,
        )
    }
    fn active(&self) -> Vec<u8> {
        fs::read(self.app.join("publisher/active.json")).unwrap()
    }
    fn reopen(&mut self) -> Publisher {
        drop(self.publisher.take());
        Publisher::open(&self.app.join("publisher"), "ads-personal-v1", None, NOW).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.publisher.take());
        let _ = fs::remove_dir_all(&self.app);
    }
}

#[test]
fn complete_archive_roundtrip_preserves_unindexed_files_gc_checkpoint_secrets_and_trust() {
    let mut f = Fixture::new();
    let old = f.publisher.as_ref().unwrap().generation.clone();
    let dir = f.app.join("publisher").join(&old);
    storage::atomic_write(&dir.join("assets/unindexed"), b"unindexed bytes").unwrap();
    storage::create_private_directory(&dir.join("cleanup-gc")).unwrap();
    storage::atomic_write(&dir.join("cleanup-gc/quarantine"), b"quarantined bytes").unwrap();
    storage::atomic_write(
        &dir.join("cleanup-checkpoint.json"),
        b"retained checkpoint bytes",
    )
    .unwrap();
    let session = f.prepare().unwrap();
    let view = session.preview();
    let encrypted = fs::read(f.app.join("publisher/backups").join(&view.backup_filename)).unwrap();
    let plaintext = backup::open(&encrypted, PASSWORD).unwrap();
    let complete: CompleteArchive = signed::strict_json(&plaintext, backup::LIMIT).unwrap();
    complete.validate().unwrap();
    assert!(complete.trust.is_some());
    assert_eq!(complete.archive.files.len(), 4);
    assert_eq!(
        complete.archive.secrets.preview_credential,
        b"synthetic-preview"
    );
    session.confirm(&view.digest, NOW, &mut f.vault).unwrap();
    let p = f.reopen();
    assert_ne!(p.generation, old);
    assert_eq!(
        fs::read(dir.join("assets/unindexed")).unwrap(),
        b"unindexed bytes"
    );
    assert!(f.vault.load(&old).is_ok());
    assert!(p.state.pending.is_none());
    assert!(p.state.history.is_empty());
    assert_eq!(p.state.schema, 2);
    assert!(backup::open(&encrypted, b"incorrect-passphrase").is_err());
    drop(p);
    let mut target = Fixture::new();
    let active = target.active();
    target
        .publisher
        .as_ref()
        .unwrap()
        .import_retained_generation(&encrypted, PASSWORD, &mut target.vault)
        .unwrap();
    assert_eq!(target.active(), active);
    assert_eq!(
        fs::read(
            target
                .app
                .join("publisher")
                .join(old)
                .join("cleanup-gc/quarantine")
        )
        .unwrap(),
        b"quarantined bytes"
    );
}

#[test]
fn cancel_releases_both_locks_and_keeps_verified_backup_and_active_generation() {
    let mut f = Fixture::new();
    let active = f.active();
    let session = f.prepare().unwrap();
    let view = session.preview();
    assert!(Publisher::open(&f.app.join("publisher"), "ads-personal-v1", None, NOW).is_err());
    assert!(
        LocalObjectStore::open(
            &f.app,
            AdChannel::Preview,
            &LocalTrust {
                schema: 1,
                key_id: "ads-personal-v1".into(),
                public_key: SigningKey::from_bytes(&[37; 32]).verifying_key().to_bytes()
            }
        )
        .is_err()
    );
    drop(session);
    assert_eq!(f.active(), active);
    drop(f.reopen());
    drop(f.local());
    assert!(
        f.app
            .join("publisher/backups")
            .join(view.backup_filename)
            .is_file()
    );
}

#[test]
fn confirmation_rejects_stale_digest_backup_source_vault_and_directory_changes() {
    for case in [
        "digest",
        "backup",
        "asset",
        "trust",
        "vault",
        "directory",
        "current",
    ] {
        let mut f = Fixture::new();
        let active = f.active();
        let session = f.prepare().unwrap();
        let view = session.preview();
        let root = f.app.join("publisher");
        match case {
            "backup" => storage::atomic_write(
                &root.join("backups").join(&view.backup_filename),
                b"corrupt",
            )
            .unwrap(),
            "asset" => {
                storage::atomic_write(&root.join(&view.generation).join("assets/late"), b"changed")
                    .unwrap()
            }
            "trust" => storage::atomic_write(&root.join("trust.json"), b"{}").unwrap(),
            "vault" => {
                let mut secrets = f.vault.load(&view.generation).unwrap();
                secrets.preview_credential.push(1);
                f.vault.store(&view.generation, &secrets).unwrap();
            }
            "directory" => {
                fs::rename(root.join(&view.generation), root.join("moved-generation")).unwrap();
                storage::create_private_directory(&root.join(&view.generation)).unwrap();
            }
            "current" => storage::atomic_write(
                &switcher_core::ad_runtime::local::directory(&f.app).join("preview/current.json"),
                b"invalid-signature",
            )
            .unwrap(),
            _ => (),
        }
        assert!(
            session
                .confirm(
                    if case == "digest" {
                        "stale"
                    } else {
                        &view.digest
                    },
                    NOW,
                    &mut f.vault
                )
                .is_err(),
            "{case}"
        );
        assert_eq!(f.active(), active, "{case}");
        assert!(f.vault.load(&view.generation).is_ok());
    }
}

#[test]
fn unsafe_files_missing_vault_and_invalid_password_prevent_preparation() {
    for case in ["symlink", "hardlink", "unknown", "vault", "password"] {
        let mut f = Fixture::new();
        let active = f.active();
        let p = f.publisher.as_ref().unwrap();
        let root = f.app.join("publisher").join(&p.generation);
        match case {
            "symlink" => {
                std::os::unix::fs::symlink("state.json", root.join("assets/link")).unwrap()
            }
            "hardlink" => fs::hard_link(root.join("state.json"), root.join("assets/link")).unwrap(),
            "unknown" => storage::atomic_write(&root.join("unknown"), b"retain me").unwrap(),
            "vault" => {
                f.vault.items.clear();
            }
            _ => (),
        }
        let local = f.local();
        let p = f.publisher.take().unwrap();
        let password = if case == "password" {
            b"short".to_vec()
        } else {
            PASSWORD.to_vec()
        };
        assert!(
            PreparedRecovery::prepare(p, local, Zeroizing::new(password), &f.vault, NOW).is_err(),
            "{case}"
        );
        assert_eq!(f.active(), active);
    }
}

#[test]
fn exclusive_vault_failure_and_post_commit_unknown_preserve_old_generation() {
    for after_commit in [false, true] {
        let mut f = Fixture::new();
        let active = f.active();
        let session = f.prepare().unwrap();
        let view = session.preview();
        f.vault.fail = !after_commit;
        ACTIVE_AFTER_COMMIT_FAULT.with(|fault| fault.set(after_commit));
        assert!(session.confirm(&view.digest, NOW, &mut f.vault).is_err());
        assert_eq!(f.active() == active, !after_commit);
        assert!(
            f.app
                .join("publisher")
                .join(view.generation.clone())
                .join("state.json")
                .is_file()
        );
        assert!(f.vault.load(&view.generation).is_ok());
        f.vault.fail = false;
        drop(f.reopen());
    }
}

#[test]
fn complete_manifest_tampering_is_rejected_before_any_import() {
    let mut f = Fixture::new();
    let session = f.prepare().unwrap();
    let view = session.preview();
    let bytes = fs::read(f.app.join("publisher/backups").join(view.backup_filename)).unwrap();
    let plaintext = backup::open(&bytes, PASSWORD).unwrap();
    let mut complete: CompleteArchive = signed::strict_json(&plaintext, backup::LIMIT).unwrap();
    complete.manifest[0].length += 1;
    let altered = backup::seal(
        &Zeroizing::new(encode(&complete, backup::LIMIT).unwrap()),
        PASSWORD,
    )
    .unwrap();
    drop(session);
    let mut target = Fixture::new();
    let active = target.active();
    assert!(
        target
            .publisher
            .as_ref()
            .unwrap()
            .import_retained_generation(&altered, PASSWORD, &mut target.vault)
            .is_err()
    );
    assert_eq!(target.active(), active);
    assert!(!target.app.join("publisher").join(view.generation).exists());
}

#[test]
fn legacy_state_with_omitted_default_fields_can_be_archived_without_rewriting_original() {
    let mut f = Fixture::new();
    let p = f.publisher.as_ref().unwrap();
    let path = f
        .app
        .join("publisher")
        .join(&p.generation)
        .join("state.json");
    let mut state: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state.as_object_mut().unwrap().remove("other_targets");
    let original = serde_json::to_vec_pretty(&state).unwrap();
    storage::atomic_write(&path, &original).unwrap();
    let session = f.prepare().unwrap();
    let view = session.preview();
    session.confirm(&view.digest, NOW, &mut f.vault).unwrap();
    let publisher = f.reopen();
    publisher
        .export_retained_generation(&view.generation, PASSWORD, &f.vault)
        .unwrap();
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn recovery_revision_exceeds_inactive_target_watermarks_without_copying_their_ledgers() {
    let mut f = Fixture::new();
    let p = f.publisher.as_mut().unwrap();
    let mut state = p.state.clone();
    state.schema = 3;
    state.target = Some(format!("local:{}", "a".repeat(64)));
    let mut ledger = state.active_ledger();
    ledger.sequences[0]
        .remember(900, signed::digest(b"synthetic-old-current"))
        .unwrap();
    state
        .other_targets
        .insert(format!("local:{}", "b".repeat(64)), ledger);
    p.persist(state).unwrap();
    let session = f.prepare().unwrap();
    let view = session.preview();
    session.confirm(&view.digest, NOW, &mut f.vault).unwrap();
    let p = f.reopen();
    assert_eq!(p.state.draft.content_revision, 901);
    assert!(p.state.other_targets.is_empty());
    assert_eq!(p.state.sequences[0].highest, 0);
    let old: State = signed::strict_json(
        &fs::read(
            f.app
                .join("publisher")
                .join(view.generation)
                .join("state.json"),
        )
        .unwrap(),
        STATE_LIMIT,
    )
    .unwrap();
    assert_eq!(old.other_targets.len(), 1);
}

#[test]
fn backup_sync_failure_and_last_moment_source_mutation_prevent_switch() {
    for fault in [1, 2] {
        let mut f = Fixture::new();
        let active = f.active();
        cleanup::CREATE_FILE_SYNC_FAULT.with(|value| value.set(fault));
        assert!(f.prepare().is_err());
        assert_eq!(f.active(), active);
        drop(f.reopen());
    }
    struct MutatingVault<'a> {
        inner: &'a mut MemoryVault,
        path: PathBuf,
    }
    impl Vault for MutatingVault<'_> {
        fn load(&self, generation: &str) -> Result<Secrets> {
            self.inner.load(generation)
        }
        fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
            self.inner.store(generation, secrets)
        }
        fn create(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
            self.inner.create(generation, secrets)?;
            storage::atomic_write(&self.path, b"changed after key creation")?;
            Ok(())
        }
    }
    let mut f = Fixture::new();
    let active = f.active();
    let session = f.prepare().unwrap();
    let view = session.preview();
    let path = f
        .app
        .join("publisher")
        .join(&view.generation)
        .join("assets/late-change");
    let mut vault = MutatingVault {
        inner: &mut f.vault,
        path,
    };
    assert!(session.confirm(&view.digest, NOW, &mut vault).is_err());
    assert_eq!(f.active(), active);
    assert!(f.vault.load(&view.generation).is_ok());
}

#[test]
fn aggregate_archive_budget_rejects_oversized_generation_without_switching() {
    let mut f = Fixture::new();
    let active = f.active();
    let p = f.publisher.as_ref().unwrap();
    let directory = f.app.join("publisher").join(&p.generation).join("assets");
    for index in 0..17 {
        storage::atomic_write(
            &directory.join(format!("unindexed-{index}")),
            &vec![1; ads::MAX_IMAGE_BYTES],
        )
        .unwrap();
    }
    assert!(matches!(f.prepare(), Err(Error::Invalid("archive_size"))));
    assert_eq!(f.active(), active);
}

#[test]
fn missing_or_corrupt_indexed_asset_never_yields_a_ready_recovery() {
    for corrupt in [false, true] {
        let mut f = Fixture::new();
        let p = f.publisher.as_mut().unwrap();
        let image = p
            .import_image(
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../tests/native-ipc-business/backup-source.png"
                )),
                "image/png",
            )
            .unwrap();
        let path = f
            .app
            .join("publisher")
            .join(&p.generation)
            .join("assets")
            .join(&image.sha256);
        if corrupt {
            storage::atomic_write(&path, b"corrupt indexed image").unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        let active = f.active();
        assert!(f.prepare().is_err());
        assert_eq!(f.active(), active);
        assert!(!f.app.join("publisher/backups").exists());
    }
}
