use super::super::{
    protocol::{Object, StoreError},
    test_support::MemoryVault,
};
use super::*;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
use switcher_core::storage;

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
}
impl ObjectStore for TestRemote {
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
        unknown,
    }))
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
