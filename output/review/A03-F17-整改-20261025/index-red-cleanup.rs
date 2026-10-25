//! Explicit, bounded local maintenance. This preparatory module is not wired
//! into Publisher yet; no index or filesystem mutation is implemented here.
use super::{
    ASSET_LIMIT, Error, HISTORY_LIMIT, Publisher, Result, STATE_LIMIT, channel_index, encode,
    protocol::{Object, StoreError},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
use switcher_core::{ads, signed, storage};
use switcher_domain::ads::{AdChannel, AdManifest, ImageAsset};

pub(crate) const DEFAULT_RETAIN: usize = 128;
pub(super) const CHECKPOINT_FILE: &str = "cleanup-checkpoint.json";

/// The production adapter must hold ONE publication-tree lifecycle lock for
/// this pair and the entire preview/commit operation. Never open two stores.
pub(crate) trait FixedCurrents {
    fn read_currents(&mut self) -> std::result::Result<[Option<Object>; 2], StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CleanupOptions {
    pub retain: usize,
    pub release_resolution: bool,
}
impl Default for CleanupOptions {
    fn default() -> Self {
        Self { retain: DEFAULT_RETAIN, release_resolution: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CurrentStamp {
    pub wire_hash: String,
    pub etag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProtectedAsset {
    pub asset: ImageAsset,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileStamp {
    pub dev: u64,
    pub ino: u64,
    pub uid: u32,
    pub mode: u32,
    pub nlink: u64,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GcIdentity {
    pub asset: ImageAsset,
    // None means absent at the explicitly confirmed GC preview.
    pub identity: Option<FileStamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CleanupPhase { Indexed, GcStarted }

/// Small State reference, never an embedded old State. The one fixed snapshot
/// is bounded by STATE_LIMIT and is authenticated by checkpoint_hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CleanupBatch {
    pub schema: u32,
    pub id: String,
    pub generation: String,
    pub checkpoint_hash: String,
    pub phase: CleanupPhase,
    pub candidates: Vec<ImageAsset>,
    pub gc_identities: Vec<GcIdentity>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CleanupPreview {
    pub digest: String,
    pub options: CleanupOptions,
    pub keep_history: Vec<usize>,
    pub removed_history: usize,
    pub protected_assets: Vec<ProtectedAsset>,
    pub candidates: Vec<ImageAsset>,
    pub candidate_bytes: u64,
    pub pending_blocks_commit: bool,
    pub currents: [Option<CurrentStamp>; 2],
}

pub(super) struct Retention {
    pub preview: CleanupPreview,
    pub sequences: [signed::Sequence; 2],
}

fn registered<'a>(publisher: &'a Publisher, asset: &ImageAsset) -> Result<&'a ImageAsset> {
    publisher.state.assets.iter().find(|stored| *stored == asset)
        .ok_or(Error::Invalid("cleanup_unregistered_reference"))
}

fn check_references(publisher: &Publisher, manifest: &AdManifest) -> Result<()> {
    for campaign in manifest.slots.iter().flat_map(|slot| &slot.campaigns) {
        registered(publisher, &campaign.image)?;
    }
    Ok(())
}

fn protect(
    publisher: &Publisher,
    manifest: &AdManifest,
    reason: &str,
    protected: &mut BTreeMap<String, ProtectedAsset>,
) -> Result<()> {
    for campaign in manifest.slots.iter().flat_map(|slot| &slot.campaigns) {
        let asset = registered(publisher, &campaign.image)?;
        let entry = protected.entry(asset.sha256.clone()).or_insert_with(|| ProtectedAsset {
            asset: asset.clone(), reasons: vec![],
        });
        if !entry.reasons.iter().any(|existing| existing == reason) {
            entry.reasons.push(reason.into());
        }
    }
    Ok(())
}

/// Read-only index planning. It deliberately does not open asset files or
/// export a backup: malformed material must not make index repair impossible.
pub(super) fn retention(
    publisher: &Publisher,
    objects: &[Option<Object>; 2],
    options: CleanupOptions,
    now: u64,
) -> Result<Retention> {
    publisher.validate_state(&publisher.state)?;
    if options.retain > HISTORY_LIMIT {
        return Err(Error::Invalid("cleanup_retain_limit"));
    }
    let mut protected = BTreeMap::new();
    let mut keep = BTreeSet::new();
    let mut sequences = publisher.state.sequences.clone();
    let mut currents: [Option<CurrentStamp>; 2] = [None, None];
    for channel in [AdChannel::Preview, AdChannel::Production] {
        let index = channel_index(channel);
        let Some(object) = &objects[index] else {
            if sequences[index].highest != 0 {
                return Err(Error::Invalid("cleanup_current_missing"));
            }
            continue;
        };
        if object.etag.is_empty() || object.etag.len() > 1024 || object.etag.chars().any(char::is_control) {
            return Err(Error::Invalid("cleanup_current_etag"));
        }
        // Authentication uses issued_at for expiry validation, but rejects a
        // future issue time. Expired/disabled current still pins its references.
        let (manifest, hash) = publisher.authenticated_manifest(&object.bytes, channel, now)?;
        sequences[index].remember(manifest.content_revision, hash.clone()).map_err(Error::Invalid)?;
        currents[index] = Some(CurrentStamp { wire_hash: signed::digest(&object.bytes), etag: object.etag.clone() });
        protect(publisher, &manifest, if index == 0 { "current_preview" } else { "current_production" }, &mut protected)?;
        for (position, record) in publisher.state.history.iter().enumerate() {
            if record.manifest.channel == channel && record.manifest.release_id == manifest.release_id {
                if record.payload_hash != hash || record.manifest != manifest {
                    return Err(Error::Invalid("cleanup_current_history_conflict"));
                }
                keep.insert(position);
            }
        }
        // No matching local history is legitimate for an A02 terminal outcome.
        // The current remains protected; never invent a verified HistoryRecord.
    }
    if keep.len() > options.retain {
        return Err(Error::Invalid("cleanup_retain_pinned_currents"));
    }
    for position in (0..publisher.state.history.len()).rev() {
        if keep.len() >= options.retain { break; }
        keep.insert(position);
    }
    protect(publisher, &publisher.state.draft, "draft", &mut protected)?;
    if let Some(pending) = &publisher.state.pending {
        protect(publisher, &pending.manifest, "pending", &mut protected)?;
    }
    for (position, record) in publisher.state.history.iter().enumerate() {
        check_references(publisher, &record.manifest)?;
        if keep.contains(&position) {
            protect(publisher, &record.manifest, "retained_history", &mut protected)?;
        }
    }
    if let Some(resolution) = publisher.resolution_read() {
        let (current, _) = publisher.authenticated_manifest(
            &resolution.current_wire, resolution.pending.manifest.channel, resolution.resolved_at,
        )?;
        check_references(publisher, &resolution.pending.manifest)?;
        check_references(publisher, &current)?;
        if !options.release_resolution {
            protect(publisher, &resolution.pending.manifest, "resolution_pending", &mut protected)?;
            protect(publisher, &current, "resolution_current", &mut protected)?;
        }
    }
    let candidates: Vec<_> = publisher.state.assets.iter()
        .filter(|asset| !protected.contains_key(&asset.sha256)).cloned().collect();
    if candidates.len() > ASSET_LIMIT { return Err(Error::Invalid("cleanup_candidate_limit")); }
    let mut preview = CleanupPreview {
        digest: String::new(), options,
        removed_history: publisher.state.history.len() - keep.len(),
        keep_history: keep.into_iter().collect(),
        protected_assets: protected.into_values().collect(),
        candidate_bytes: candidates.iter().map(|asset| u64::from(asset.bytes)).sum(),
        candidates, pending_blocks_commit: publisher.state.pending.is_some(), currents,
    };
    // Hash the existing State separately to avoid doubling the bounded JSON
    // allocation when it is close to STATE_LIMIT.
    let state_hash = signed::digest(&encode(&publisher.state, STATE_LIMIT)?);
    preview.digest = signed::digest(&encode(&(&publisher.generation, state_hash, &preview), STATE_LIMIT)?);
    Ok(Retention { preview, sequences })
}

/// GC-only inspection. No recursive paths, image decode, or deletion. The
/// caller supplies a registered hash from the bound batch, not a user path.
pub(super) fn inspect_candidate(directory: &Path, asset: &ImageAsset) -> Result<Option<FileStamp>> {
    ads::validate_asset(asset).map_err(Error::Invalid)?;
    storage::validate_no_symlinks(directory)?;
    let parent = fs::symlink_metadata(directory)?;
    if !parent.is_dir() || parent.uid() != unsafe { libc::geteuid() } || parent.mode() & 0o777 != 0o700 {
        return Err(Error::Invalid("cleanup_asset_directory"));
    }
    let path = directory.join(&asset.sha256);
    let file = match OpenOptions::new().read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != parent.uid() || metadata.nlink() != 1
        || metadata.mode() & 0o777 != 0o600 || metadata.len() != u64::from(asset.bytes) {
        return Err(Error::Invalid("cleanup_asset_metadata"));
    }
    let mut bytes = Vec::new();
    file.take(ads::MAX_IMAGE_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() != asset.bytes as usize || signed::digest(&bytes) != asset.sha256 {
        return Err(Error::Invalid("cleanup_asset_hash"));
    }
    let current = fs::symlink_metadata(&path)?;
    if current.dev() != metadata.dev() || current.ino() != metadata.ino()
        || current.uid() != metadata.uid() || current.mode() != metadata.mode()
        || current.nlink() != 1 || current.len() != metadata.len() {
        return Err(Error::Invalid("cleanup_asset_changed"));
    }
    Ok(Some(FileStamp { dev: metadata.dev(), ino: metadata.ino(), uid: metadata.uid(),
        mode: metadata.mode(), nlink: metadata.nlink(), bytes: metadata.len(), sha256: asset.sha256.clone() }))
}

impl Publisher {
    pub(crate) fn cleanup_preview(&self, source: &mut dyn FixedCurrents, options: CleanupOptions, now: u64) -> Result<CleanupPreview> {
        Ok(retention(self, &source.read_currents()?, options, now)?.preview)
    }
    pub(crate) fn cleanup_commit(&mut self, source: &mut dyn FixedCurrents, options: CleanupOptions, expected: &str, now: u64) -> Result<()> {
        let preview = self.cleanup_preview(source, options, now)?;
        if preview.digest != expected { return Err(Error::Invalid("cleanup_stale_preview")); }
        if preview.pending_blocks_commit { return Err(Error::PendingOperation); }
        // RED seam: retain the old no-pruning persist behavior until the
        // capacity regression has failed through this explicit entry point.
        self.persist(self.state.clone())
    }
}
