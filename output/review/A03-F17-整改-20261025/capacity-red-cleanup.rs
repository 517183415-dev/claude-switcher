//! Explicit, bounded local index maintenance and separately confirmed GC.
use super::{
    ASSET_LIMIT, Error, HISTORY_LIMIT, Publisher, Result, STATE_LIMIT, channel_index, encode,
    protocol::{Object, StoreError},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Read,
    os::fd::{AsRawFd, FromRawFd},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
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
        Self {
            retain: DEFAULT_RETAIN,
            release_resolution: false,
        }
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
    pub status: GcClass,
    pub slot: String,
    pub quarantined: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CleanupPhase {
    Indexed,
    GcStarted,
}

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
    pub previous_checkpoint: Option<FileStamp>,
}

pub(super) struct Retention {
    pub preview: CleanupPreview,
    pub sequences: [signed::Sequence; 2],
}

fn registered<'a>(publisher: &'a Publisher, asset: &ImageAsset) -> Result<&'a ImageAsset> {
    publisher
        .state
        .assets
        .iter()
        .find(|stored| *stored == asset)
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
        let entry = protected
            .entry(asset.sha256.clone())
            .or_insert_with(|| ProtectedAsset {
                asset: asset.clone(),
                reasons: vec![],
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
        if object.etag.is_empty()
            || object.etag.len() > 1024
            || object.etag.chars().any(char::is_control)
        {
            return Err(Error::Invalid("cleanup_current_etag"));
        }
        // Authentication uses issued_at for expiry validation, but rejects a
        // future issue time. Expired/disabled current still pins its references.
        let (manifest, hash) = publisher.authenticated_manifest(&object.bytes, channel, now)?;
        sequences[index]
            .remember(manifest.content_revision, hash.clone())
            .map_err(Error::Invalid)?;
        currents[index] = Some(CurrentStamp {
            wire_hash: signed::digest(&object.bytes),
            etag: object.etag.clone(),
        });
        protect(
            publisher,
            &manifest,
            if index == 0 {
                "current_preview"
            } else {
                "current_production"
            },
            &mut protected,
        )?;
        for (position, record) in publisher.state.history.iter().enumerate() {
            if record.manifest.channel == channel
                && record.manifest.release_id == manifest.release_id
            {
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
        if keep.len() >= options.retain {
            break;
        }
        keep.insert(position);
    }
    protect(publisher, &publisher.state.draft, "draft", &mut protected)?;
    if let Some(pending) = &publisher.state.pending {
        protect(publisher, &pending.manifest, "pending", &mut protected)?;
    }
    for (position, record) in publisher.state.history.iter().enumerate() {
        check_references(publisher, &record.manifest)?;
        if keep.contains(&position) {
            protect(
                publisher,
                &record.manifest,
                "retained_history",
                &mut protected,
            )?;
        }
    }
    if let Some(resolution) = publisher.resolution_read() {
        let (current, _) = publisher.authenticated_manifest(
            &resolution.current_wire,
            resolution.pending.manifest.channel,
            resolution.resolved_at,
        )?;
        check_references(publisher, &resolution.pending.manifest)?;
        check_references(publisher, &current)?;
        if !options.release_resolution {
            protect(
                publisher,
                &resolution.pending.manifest,
                "resolution_pending",
                &mut protected,
            )?;
            protect(publisher, &current, "resolution_current", &mut protected)?;
        }
    }
    let candidates: Vec<_> = publisher
        .state
        .assets
        .iter()
        .filter(|asset| !protected.contains_key(&asset.sha256))
        .cloned()
        .collect();
    if candidates.len() > ASSET_LIMIT {
        return Err(Error::Invalid("cleanup_candidate_limit"));
    }
    let mut preview = CleanupPreview {
        digest: String::new(),
        options,
        removed_history: publisher.state.history.len() - keep.len(),
        keep_history: keep.into_iter().collect(),
        protected_assets: protected.into_values().collect(),
        candidate_bytes: candidates.iter().map(|asset| u64::from(asset.bytes)).sum(),
        candidates,
        pending_blocks_commit: publisher.state.pending.is_some(),
        currents,
        previous_checkpoint: None,
    };
    // Hash the existing State separately to avoid doubling the bounded JSON
    // allocation when it is close to STATE_LIMIT.
    let state_hash = signed::digest(&encode(&publisher.state, STATE_LIMIT)?);
    preview.digest = signed::digest(&encode(
        &(&publisher.generation, state_hash, &preview),
        STATE_LIMIT,
    )?);
    Ok(Retention { preview, sequences })
}

/// GC-only inspection. No recursive paths, image decode, or deletion. The
/// caller supplies a registered hash from the bound batch, not a user path.
#[cfg(test)]
pub(super) fn inspect_candidate(directory: &Path, asset: &ImageAsset) -> Result<Option<FileStamp>> {
    inspect_at(&PrivateDirectory::open(directory)?, &asset.sha256, asset)
}

struct PrivateDirectory {
    file: File,
    path: PathBuf,
    dev: u64,
    ino: u64,
}
impl PrivateDirectory {
    fn open(path: &Path) -> Result<Self> {
        storage::validate_no_symlinks(path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(Error::Invalid("cleanup_directory"));
        }
        let directory = Self {
            file,
            path: path.into(),
            dev: metadata.dev(),
            ino: metadata.ino(),
        };
        directory.check()?;
        Ok(directory)
    }
    fn check(&self) -> Result<()> {
        storage::validate_no_symlinks(&self.path)?;
        let actual = fs::symlink_metadata(&self.path)?;
        if !actual.is_dir()
            || actual.uid() != unsafe { libc::geteuid() }
            || actual.mode() & 0o7777 != 0o700
            || (actual.dev(), actual.ino()) != (self.dev, self.ino)
        {
            return Err(Error::Invalid("cleanup_directory_changed"));
        }
        Ok(())
    }
}
fn relative_name(name: &str) -> Result<CString> {
    if name.is_empty()
        || name.len() > 100
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(Error::Invalid("cleanup_file_name"));
    }
    CString::new(name).map_err(|_| Error::Invalid("cleanup_file_name"))
}
fn open_at(directory: &PrivateDirectory, name: &str) -> Result<Option<File>> {
    directory.check()?;
    let name = relative_name(name)?;
    // SAFETY: the directory FD and NUL-terminated single-component name remain live.
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error.into());
    }
    // SAFETY: openat returned a newly owned descriptor.
    Ok(Some(unsafe { File::from_raw_fd(fd) }))
}
fn inspect_at(
    directory: &PrivateDirectory,
    name: &str,
    asset: &ImageAsset,
) -> Result<Option<FileStamp>> {
    ads::validate_asset(asset).map_err(Error::Invalid)?;
    let Some(file) = open_at(directory, name)? else {
        return Ok(None);
    };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != 0o600
        || metadata.len() != u64::from(asset.bytes)
    {
        return Err(Error::Invalid("cleanup_asset_metadata"));
    }
    let mut bytes = Vec::new();
    (&file)
        .take(ads::MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() != asset.bytes as usize || signed::digest(&bytes) != asset.sha256 {
        return Err(Error::Invalid("cleanup_asset_hash"));
    }
    let current = open_at(directory, name)?
        .ok_or(Error::Invalid("cleanup_asset_changed"))?
        .metadata()?;
    let held = file.metadata()?;
    let identity = |m: &fs::Metadata| (m.dev(), m.ino(), m.uid(), m.mode(), m.nlink(), m.len());
    if identity(&metadata) != identity(&held) || identity(&metadata) != identity(&current) {
        return Err(Error::Invalid("cleanup_asset_changed"));
    }
    Ok(Some(FileStamp {
        dev: metadata.dev(),
        ino: metadata.ino(),
        uid: metadata.uid(),
        mode: metadata.mode(),
        nlink: metadata.nlink(),
        bytes: metadata.len(),
        sha256: asset.sha256.clone(),
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GcClass {
    Eligible,
    AlreadyMissing,
    Blocked,
    Referenced,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CleanupCommit {
    pub batch_id: String,
    pub removed_history: usize,
    pub removed_assets: usize,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GcPreview {
    pub digest: String,
    pub batch_id: String,
    pub eligible: Vec<String>,
    pub already_missing: Vec<String>,
    pub blocked: Vec<String>,
    pub referenced: Vec<String>,
    pub blocked_reasons: Vec<(String, String)>,
    pub eligible_bytes: u64,
    pub pending_blocks_commit: bool,
    #[serde(skip)]
    identities: Vec<GcIdentity>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GcOutcome {
    pub deleted: Vec<String>,
    pub already_missing: Vec<String>,
    pub blocked: Vec<String>,
    pub referenced: Vec<String>,
    pub deleted_bytes: u64,
    pub batch_finished: bool,
}

fn hash_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn validate_batch(batch: Option<&CleanupBatch>, generation: &str) -> Result<()> {
    let Some(batch) = batch else {
        return Ok(());
    };
    if batch.schema != 1
        || !super::valid_generation(&batch.id)
        || batch.generation != generation
        || !hash_valid(&batch.checkpoint_hash)
        || batch.candidates.len() > ASSET_LIMIT
        || (batch.phase == CleanupPhase::Indexed && !batch.gc_identities.is_empty())
        || (batch.phase == CleanupPhase::GcStarted
            && batch.gc_identities.len() != batch.candidates.len())
    {
        return Err(Error::Invalid("cleanup_batch"));
    }
    let mut seen = BTreeSet::new();
    for asset in &batch.candidates {
        ads::validate_asset(asset).map_err(Error::Invalid)?;
        if !seen.insert(&asset.sha256) {
            return Err(Error::Invalid("cleanup_candidate_duplicate"));
        }
    }
    for (identity, asset) in batch.gc_identities.iter().zip(&batch.candidates) {
        if &identity.asset != asset
            || identity.slot != format!("{}-{}", batch.id, asset.sha256)
            || (identity.status == GcClass::Eligible && identity.identity.is_none())
        {
            return Err(Error::Invalid("cleanup_identity"));
        }
        if let Some(stamp) = &identity.identity {
            if stamp.sha256 != asset.sha256
                || stamp.bytes != u64::from(asset.bytes)
                || stamp.nlink != 1
                || stamp.mode & 0o7777 != 0o600
            {
                return Err(Error::Invalid("cleanup_identity"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
thread_local! { pub(super) static CLEANUP_WRITE_FAULT: std::cell::Cell<Option<(&'static str, u8)>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
pub(super) fn persist_fault(old: Option<&CleanupBatch>, new: Option<&CleanupBatch>) -> u8 {
    let stage = match (old, new) {
        (None, Some(_)) => "index",
        (Some(_), None) => "finish",
        (Some(old), Some(new))
            if old.phase == CleanupPhase::Indexed && new.phase == CleanupPhase::GcStarted =>
        {
            "start"
        }
        _ => return 0,
    };
    CLEANUP_WRITE_FAULT.with(|slot| match slot.get() {
        Some((at, code)) if at == stage => {
            slot.set(None);
            code
        }
        _ => 0,
    })
}

#[cfg(test)]
thread_local! { pub(super) static CLEANUP_FAULT: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
type CleanupHook = (&'static str, Box<dyn FnOnce()>);
#[cfg(test)]
thread_local! { pub(super) static CLEANUP_HOOK: std::cell::RefCell<Option<CleanupHook>> = const { std::cell::RefCell::new(None) }; }

fn fault(point: &'static str) -> Result<()> {
    #[cfg(test)]
    if let Some((_, hook)) = CLEANUP_HOOK.with(|slot| {
        if slot.borrow().as_ref().is_some_and(|(at, _)| *at == point) {
            slot.borrow_mut().take()
        } else {
            None
        }
    }) {
        hook();
    }
    #[cfg(test)]
    if CLEANUP_FAULT.with(|fault| {
        if fault.get() == Some(point) {
            fault.set(None);
            true
        } else {
            false
        }
    }) {
        return Err(Error::Io(std::io::Error::other(point)));
    }
    let _ = point;
    Ok(())
}

impl Publisher {
    pub(crate) fn cleanup_pending(&self) -> bool {
        self.state.cleanup.is_some()
    }

    fn cleanup_lock(&self) -> Result<()> {
        if !self._lock.protects(&self.root) {
            return Err(Error::Invalid("cleanup_lock"));
        }
        Ok(())
    }

    fn checkpoint(&self) -> Result<Option<(super::State, FileStamp)>> {
        let path = self.directory().join(CHECKPOINT_FILE);
        let before = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !before.is_file()
            || before.uid() != unsafe { libc::geteuid() }
            || before.mode() & 0o7777 != 0o600
            || before.nlink() != 1
            || before.len() > STATE_LIMIT as u64
        {
            return Err(Error::Invalid("cleanup_checkpoint_metadata"));
        }
        let bytes = super::read_private(&path, STATE_LIMIT)?;
        let after = fs::symlink_metadata(&path)?;
        if (
            before.dev(),
            before.ino(),
            before.mode(),
            before.uid(),
            before.nlink(),
            before.len(),
        ) != (
            after.dev(),
            after.ino(),
            after.mode(),
            after.uid(),
            after.nlink(),
            after.len(),
        ) {
            return Err(Error::Invalid("cleanup_checkpoint_changed"));
        }
        let state: super::State =
            signed::strict_json(&bytes, STATE_LIMIT).map_err(Error::Invalid)?;
        if state.cleanup.is_some() {
            return Err(Error::Invalid("cleanup_checkpoint_nested"));
        }
        self.validate_state(&state)?;
        Ok(Some((
            state,
            FileStamp {
                dev: after.dev(),
                ino: after.ino(),
                uid: after.uid(),
                mode: after.mode(),
                nlink: after.nlink(),
                bytes: after.len(),
                sha256: signed::digest(&bytes),
            },
        )))
    }

    fn prepare_cleanup(
        &self,
        source: &mut dyn FixedCurrents,
        options: CleanupOptions,
        now: u64,
    ) -> Result<Retention> {
        self.cleanup_lock()?;
        if self.state.cleanup.is_some() {
            return Err(Error::Invalid("cleanup_finish_existing_batch"));
        }
        let mut result = retention(self, &source.read_currents()?, options, now)?;
        result.preview.previous_checkpoint = self.checkpoint()?.map(|(_, stamp)| stamp);
        result.preview.digest.clear();
        result.preview.digest = signed::digest(&encode(
            &(
                &self.generation,
                signed::digest(&encode(&self.state, STATE_LIMIT)?),
                &result.preview,
            ),
            STATE_LIMIT,
        )?);
        Ok(result)
    }

    pub(crate) fn cleanup_preview(
        &self,
        source: &mut dyn FixedCurrents,
        options: CleanupOptions,
        now: u64,
    ) -> Result<CleanupPreview> {
        Ok(self.prepare_cleanup(source, options, now)?.preview)
    }

    pub(crate) fn cleanup_commit(
        &mut self,
        source: &mut dyn FixedCurrents,
        options: CleanupOptions,
        expected: &str,
        now: u64,
    ) -> Result<CleanupCommit> {
        let prepared = self.prepare_cleanup(source, options, now)?;
        let preview = prepared.preview;
        if preview.digest != expected {
            return Err(Error::Invalid("cleanup_stale_preview"));
        }
        if preview.pending_blocks_commit {
            return Err(Error::PendingOperation);
        }
        let checkpoint = encode(&self.state, STATE_LIMIT)?;
        let mut next = self.state.clone();
        let keep: BTreeSet<_> = preview.keep_history.iter().copied().collect();
        next.history = next
            .history
            .into_iter()
            .enumerate()
            .filter_map(|(index, record)| keep.contains(&index).then_some(record))
            .collect();
        let candidates: BTreeSet<_> = preview
            .candidates
            .iter()
            .map(|asset| asset.sha256.clone())
            .collect();
        next.assets
            .retain(|asset| !candidates.contains(&asset.sha256));
        next.sequences = prepared.sequences;
        if preview.options.release_resolution {
            next.last_resolution = None;
        }
        let batch_id = super::random_id()?;
        next.cleanup = Some(CleanupBatch {
            schema: 1,
            id: batch_id.clone(),
            generation: self.generation.clone(),
            checkpoint_hash: signed::digest(&checkpoint),
            phase: CleanupPhase::Indexed,
            candidates: preview.candidates.clone(),
            gc_identities: vec![],
        });
        // Check the small reference fits before publishing the checkpoint.
        self.validate_state(&next)?;
        encode(&next, STATE_LIMIT)?;
        fault("checkpoint_before")?;
        storage::atomic_write(&self.directory().join(CHECKPOINT_FILE), &checkpoint)?;
        fault("checkpoint_after")?;
        fault("index_before")?;
        self.persist(next)?;
        fault("index_after")?;
        Ok(CleanupCommit {
            batch_id,
            removed_history: preview.removed_history,
            removed_assets: candidates.len(),
        })
    }

    fn bound_checkpoint(&self) -> Result<()> {
        let batch = self
            .state
            .cleanup
            .as_ref()
            .ok_or(Error::Invalid("cleanup_batch_missing"))?;
        validate_batch(Some(batch), &self.generation)?;
        let (old, stamp) = self
            .checkpoint()?
            .ok_or(Error::Invalid("cleanup_checkpoint_missing"))?;
        if stamp.sha256 != batch.checkpoint_hash
            || batch
                .candidates
                .iter()
                .any(|asset| !old.assets.contains(asset))
        {
            return Err(Error::Invalid("cleanup_checkpoint_binding"));
        }
        Ok(())
    }

    pub(crate) fn gc_preview(&self, source: &mut dyn FixedCurrents, now: u64) -> Result<GcPreview> {
        self.cleanup_lock()?;
        self.bound_checkpoint()?;
        let batch = self
            .state
            .cleanup
            .as_ref()
            .ok_or(Error::Invalid("cleanup_batch_missing"))?;
        let references = retention(
            self,
            &source.read_currents()?,
            CleanupOptions {
                retain: HISTORY_LIMIT,
                release_resolution: false,
            },
            now,
        )?;
        let protected: BTreeSet<_> = references
            .preview
            .protected_assets
            .iter()
            .map(|item| item.asset.sha256.as_str())
            .chain(self.state.assets.iter().map(|asset| asset.sha256.as_str()))
            .collect();
        let mut preview = GcPreview {
            digest: String::new(),
            batch_id: batch.id.clone(),
            eligible: vec![],
            already_missing: vec![],
            blocked: vec![],
            referenced: vec![],
            blocked_reasons: vec![],
            eligible_bytes: 0,
            pending_blocks_commit: self.state.pending.is_some(),
            identities: vec![],
        };
        let assets_directory = PrivateDirectory::open(&self.directory().join("assets"))?;
        let quarantine = match PrivateDirectory::open(&self.directory().join("cleanup-gc")) {
            Ok(directory) => Some(directory),
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        for (index, asset) in batch.candidates.iter().enumerate() {
            let slot = format!("{}-{}", batch.id, asset.sha256);
            let original = batch
                .gc_identities
                .get(index)
                .and_then(|item| item.identity.clone());
            let mut quarantined = false;
            let (status, identity) = if protected.contains(asset.sha256.as_str()) {
                preview.referenced.push(asset.sha256.clone());
                (GcClass::Referenced, original)
            } else {
                let inspected = (|| {
                    fault("gc_inspect_io")?;
                    if let Some(directory) = &quarantine {
                        if let Some(stamp) = inspect_at(directory, &slot, asset)? {
                            quarantined = true;
                            if original.as_ref() != Some(&stamp) {
                                return Err(Error::Invalid("cleanup_quarantine_identity"));
                            }
                            return Ok(Some(stamp));
                        }
                    }
                    let current = inspect_at(&assets_directory, &asset.sha256, asset)?;
                    if let (Some(expected), Some(actual)) = (&original, &current) {
                        if expected != actual {
                            return Err(Error::Invalid("cleanup_source_identity"));
                        }
                    }
                    Ok(current)
                })();
                match inspected {
                    Ok(Some(stamp)) => {
                        preview.eligible.push(asset.sha256.clone());
                        preview.eligible_bytes += stamp.bytes;
                        (GcClass::Eligible, Some(stamp))
                    }
                    Ok(None) => {
                        preview.already_missing.push(asset.sha256.clone());
                        (GcClass::AlreadyMissing, original)
                    }
                    Err(Error::Io(error)) if error.raw_os_error() != Some(libc::ELOOP) => {
                        return Err(error.into());
                    }
                    Err(error) => {
                        preview.blocked.push(asset.sha256.clone());
                        preview
                            .blocked_reasons
                            .push((asset.sha256.clone(), error.to_string()));
                        (GcClass::Blocked, original)
                    }
                }
            };
            preview.identities.push(GcIdentity {
                asset: asset.clone(),
                identity,
                status,
                slot,
                quarantined,
            });
        }
        preview.digest = signed::digest(&encode(
            &(
                &self.generation,
                signed::digest(&encode(&self.state, STATE_LIMIT)?),
                references.preview.currents,
                &preview,
                &preview.identities,
                self.checkpoint()?.map(|(_, stamp)| stamp),
            ),
            STATE_LIMIT,
        )?);
        Ok(preview)
    }

    pub(crate) fn gc_commit(
        &mut self,
        source: &mut dyn FixedCurrents,
        expected: &str,
        now: u64,
    ) -> Result<GcOutcome> {
        let preview = self.gc_preview(source, now)?;
        if preview.digest != expected {
            return Err(Error::Invalid("cleanup_stale_gc_preview"));
        }
        if preview.pending_blocks_commit {
            return Err(Error::PendingOperation);
        }
        let mut next = self.state.clone();
        let batch = next
            .cleanup
            .as_mut()
            .ok_or(Error::Invalid("cleanup_batch_missing"))?;
        batch.phase = CleanupPhase::GcStarted;
        batch.gc_identities = preview.identities.clone();
        fault("gc_start_before")?;
        self.persist(next)?;
        fault("gc_start_after")?;
        let assets = PrivateDirectory::open(&self.directory().join("assets"))?;
        let quarantine_path = self.directory().join("cleanup-gc");
        match fs::symlink_metadata(&quarantine_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                storage::create_private_directory(&quarantine_path)?;
                storage::sync_directory(&self.directory())?;
            }
            Err(error) => return Err(error.into()),
        }
        let quarantine = PrivateDirectory::open(&quarantine_path)?;
        let mut outcome = GcOutcome {
            deleted: vec![],
            deleted_bytes: 0,
            already_missing: preview.already_missing,
            blocked: preview.blocked,
            referenced: preview.referenced,
            batch_finished: false,
        };
        for item in &preview.identities {
            if item.status != GcClass::Eligible {
                continue;
            }
            self.cleanup_lock()?;
            // Both lifecycle locks remain held. Authenticate again before each
            // candidate, including a slot left by an interrupted earlier run.
            let live = retention(
                self,
                &source.read_currents()?,
                CleanupOptions {
                    retain: HISTORY_LIMIT,
                    release_resolution: false,
                },
                now,
            )?;
            if self.state.assets.contains(&item.asset)
                || live
                    .preview
                    .protected_assets
                    .iter()
                    .any(|entry| entry.asset.sha256 == item.asset.sha256)
            {
                outcome.referenced.push(item.asset.sha256.clone());
                continue;
            }
            let expected = item
                .identity
                .as_ref()
                .ok_or(Error::Invalid("cleanup_identity"))?;
            if !item.quarantined {
                if inspect_at(&assets, &item.asset.sha256, &item.asset)?.as_ref() != Some(expected)
                {
                    return Err(Error::Invalid("cleanup_source_changed"));
                }
                assets.check()?;
                quarantine.check()?;
                let from = relative_name(&item.asset.sha256)?;
                let to = relative_name(&item.slot)?;
                fault("gc_rename_before")?;
                // SAFETY: both trusted directory FDs and single-component names
                // remain live. EXCL never overwrites an existing quarantine slot.
                let renamed = unsafe {
                    libc::renameatx_np(
                        assets.file.as_raw_fd(),
                        from.as_ptr(),
                        quarantine.file.as_raw_fd(),
                        to.as_ptr(),
                        libc::RENAME_EXCL,
                    )
                };
                if renamed != 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                fault("gc_rename_after")?;
            }
            fault("gc_sync_assets")?;
            assets.file.sync_all()?;
            fault("gc_sync_quarantine")?;
            quarantine.file.sync_all()?;
            fault("gc_verify_before")?;
            if inspect_at(&quarantine, &item.slot, &item.asset)?.as_ref() != Some(expected) {
                return Err(Error::Invalid("cleanup_quarantine_changed"));
            }
            self.cleanup_lock()?;
            assets.check()?;
            quarantine.check()?;
            let name = relative_name(&item.slot)?;
            // SAFETY: only the exact, revalidated quarantine name is unlinked.
            // Never unlink the original hash path, which may have been recreated.
            if unsafe { libc::unlinkat(quarantine.file.as_raw_fd(), name.as_ptr(), 0) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            fault("gc_unlink_after")?;
            quarantine.file.sync_all()?;
            outcome.deleted.push(item.asset.sha256.clone());
            outcome.deleted_bytes += expected.bytes;
        }
        let mut next = self.state.clone();
        next.cleanup = None;
        fault("gc_finish_before")?;
        self.persist(next)?;
        fault("gc_finish_after")?;
        outcome.batch_finished = true;
        Ok(outcome)
    }
}
