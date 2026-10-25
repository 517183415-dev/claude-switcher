//! Preparatory tests; intentionally not registered in mod.rs until the owner
//! releases that file. These have not been compiled or executed.
use super::{cleanup::*, *};
use ed25519_dalek::Signer;
use std::os::unix::fs::PermissionsExt;

struct CleanupRoot(PathBuf);
impl CleanupRoot {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-cleanup-{}-{}", std::process::id(), random_id().unwrap(),
        )))
    }
}
impl Drop for CleanupRoot {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn cleanup_publisher(root: &CleanupRoot) -> Publisher {
    Publisher::open(&root.0, "cleanup-test", Some(cleanup_signer().verifying_key()), 1000).unwrap()
}
fn cleanup_signer() -> SigningKey { SigningKey::from_bytes(&[0x37; 32]) }
fn cleanup_object(manifest: &AdManifest) -> protocol::Object {
    let payload = String::from_utf8(encode(manifest, ads::MAX_MANIFEST_BYTES).unwrap()).unwrap();
    let bytes = serde_json::to_vec(&signed::Envelope {
        signature: cleanup_signer().sign(payload.as_bytes()).to_bytes().to_vec(), payload,
    }).unwrap();
    protocol::Object { etag: signed::digest(&bytes), bytes }
}
fn cleanup_asset(bytes: &[u8]) -> ImageAsset {
    ImageAsset { sha256: signed::digest(bytes), mime: "image/png".into(), bytes: bytes.len() as u32, width: 1, height: 1 }
}

#[test]
fn f17_retention_full_capacity_pins_currents_inside_default_128() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    for index in 0..HISTORY_LIMIT {
        let mut manifest = publisher.state.draft.clone();
        manifest.release_id = format!("history-{index}");
        manifest.content_revision = index as u64 + 1;
        manifest.channel = if index % 2 == 0 { AdChannel::Preview } else { AdChannel::Production };
        publisher.state.history.push(HistoryRecord {
            payload_hash: signed::digest(&encode(&manifest, ads::MAX_MANIFEST_BYTES).unwrap()),
            manifest, published_at: 1000,
        });
    }
    // No files exist for these synthetic registered assets. Index preview must
    // not depend on a successful backup or a full asset read/decode.
    publisher.state.assets = (0..ASSET_LIMIT).map(|index| cleanup_asset(format!("asset-{index}").as_bytes())).collect();
    let objects = [Some(cleanup_object(&publisher.state.history[0].manifest)), Some(cleanup_object(&publisher.state.history[1].manifest))];
    let before = encode(&publisher.state, STATE_LIMIT).unwrap();
    let result = retention(&publisher, &objects, CleanupOptions::default(), 1000).unwrap();
    assert_eq!(result.preview.keep_history.len(), 128);
    assert!(result.preview.keep_history.contains(&0));
    assert!(result.preview.keep_history.contains(&1));
    assert_eq!(&result.preview.keep_history[2..], &(130..256).collect::<Vec<_>>());
    assert_eq!(result.preview.removed_history, 128);
    assert_eq!(result.preview.candidates.len(), 128);
    assert_eq!(encode(&publisher.state, STATE_LIMIT).unwrap(), before);
    assert!(retention(&publisher, &objects, CleanupOptions { retain: 1, release_resolution: false }, 1000).is_err());
    assert!(retention(&publisher, &objects, CleanupOptions { retain: 257, release_resolution: false }, 1000).is_err());
    let all = retention(&publisher, &objects, CleanupOptions { retain: 256, release_resolution: false }, 1000).unwrap();
    assert_eq!(all.preview.removed_history, 0);
}

#[test]
fn f17_current_without_history_does_not_create_verified_history_and_expiry_is_allowed() {
    let root = CleanupRoot::new();
    let publisher = cleanup_publisher(&root);
    let current = cleanup_object(&publisher.state.draft);
    let result = retention(&publisher, &[Some(current), None], CleanupOptions::default(), publisher.state.draft.expires_at + 1).unwrap();
    assert!(result.preview.keep_history.is_empty());
    assert_eq!(publisher.state.history.len(), 0);
    assert_eq!(result.sequences[0].highest, publisher.state.draft.content_revision);
}

#[test]
fn f17_current_watermark_authentication_and_stale_preview_binding() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    let object = cleanup_object(&publisher.state.draft);
    let original = retention(&publisher, &[Some(object.clone()), None], CleanupOptions::default(), 1000).unwrap();
    publisher.state.sequences = original.sequences;
    assert!(retention(&publisher, &[None, None], CleanupOptions::default(), 1000).is_err());
    let mut changed = publisher.state.draft.clone();
    changed.release_id = "same-sequence-different-payload".into();
    assert!(retention(&publisher, &[Some(cleanup_object(&changed)), None], CleanupOptions::default(), 1000).is_err());
    changed.content_revision += 1;
    changed.issued_at = 1001;
    assert!(retention(&publisher, &[Some(cleanup_object(&changed)), None], CleanupOptions::default(), 1000).is_err());
    let mut bad = object.clone();
    bad.bytes[0] ^= 1;
    assert!(retention(&publisher, &[Some(bad), None], CleanupOptions::default(), 1000).is_err());
    let baseline = retention(&publisher, &[Some(object.clone()), None], CleanupOptions::default(), 1000).unwrap();
    let mut etag = object.clone(); etag.etag.push('x');
    let other = retention(&publisher, &[Some(etag), None], CleanupOptions::default(), 1000).unwrap();
    assert_ne!(baseline.preview.digest, other.preview.digest);
    publisher.state.recovery_required = !publisher.state.recovery_required;
    let changed_state = retention(&publisher, &[Some(object), None], CleanupOptions::default(), 1000).unwrap();
    assert_ne!(baseline.preview.digest, changed_state.preview.digest);
}

#[test]
fn f17_pending_allows_read_only_preview_and_marks_execution_blocked() {
    let root = CleanupRoot::new();
    let mut publisher = cleanup_publisher(&root);
    publisher.state.pending = Some(Pending {
        manifest: publisher.state.draft.clone(), wire: cleanup_object(&publisher.state.draft).bytes,
        expected_etag: None, phase: Phase::Prepared,
    });
    let before = encode(&publisher.state, STATE_LIMIT).unwrap();
    let preview = retention(&publisher, &[None, None], CleanupOptions::default(), 1000).unwrap().preview;
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
    assert_ne!(inspect_candidate(&directory, &asset).unwrap().unwrap().ino, identity.ino);
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&alias, &path).unwrap();
    assert!(inspect_candidate(&directory, &asset).is_err());
    assert!(fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
}
