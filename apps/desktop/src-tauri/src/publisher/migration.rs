//! Explicit migration to a verified local publication target.
use super::*;
use cleanup::{FixedCurrents, PrivateDirectory};

pub(crate) mod recovery;

#[cfg(test)]
thread_local! { pub(super) static ACTIVE_AFTER_COMMIT_FAULT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[derive(Serialize)]
pub(crate) struct MigrationPreview {
    pub digest: String,
}
#[derive(Serialize)]
struct Evidence<'a> {
    generation: &'a str,
    state_hash: String,
    target: &'a str,
    currents: [Option<(String, String)>; 2],
}
struct Prepared {
    preview: MigrationPreview,
    target: String,
    sequences: [signed::Sequence; 2],
    highest: u64,
}
impl Publisher {
    fn prepare_migration(&self, source: &mut dyn FixedCurrents, now: u64) -> Result<Prepared> {
        self.require_no_remote_intents()?;
        if !self._lock.protects(&self.root) {
            return Err(Error::Invalid("migration_lock"));
        }
        self.validate_state(&self.state)?;
        let target = source
            .target_identity()
            .ok_or(Error::Invalid("publisher_target_unavailable"))?;
        if !target.starts_with("local:") {
            return Err(Error::Invalid("publisher_target_unavailable"));
        }
        let objects = source.read_currents().map_err(Error::Store)?;
        if source.target_identity().as_deref() != Some(target.as_str()) {
            return Err(Error::Invalid("migration_target_changed"));
        }
        let mut sequences = [signed::Sequence::default(), signed::Sequence::default()];
        let mut currents: [Option<(String, String)>; 2] = [None, None];
        for channel in [AdChannel::Preview, AdChannel::Production] {
            let index = channel_index(channel);
            if let Some(object) = &objects[index] {
                if object.etag.is_empty()
                    || object.etag.len() > 1024
                    || object.etag.chars().any(char::is_control)
                {
                    return Err(Error::Invalid("migration_current_etag"));
                }
                let (manifest, hash) = self.authenticated_manifest(&object.bytes, channel, now)?;
                sequences[index]
                    .remember(manifest.content_revision, hash)
                    .map_err(Error::Invalid)?;
                currents[index] = Some((signed::digest(&object.bytes), object.etag.clone()));
            }
        }
        let mut highest = self.state.draft.content_revision;
        for sequence in self.state.sequences.iter().chain(sequences.iter()).chain(
            self.state
                .other_targets
                .values()
                .flat_map(|ledger| ledger.sequences.iter()),
        ) {
            highest = highest.max(sequence.highest);
        }
        for record in &self.state.history {
            highest = highest.max(record.manifest.content_revision);
        }
        if let Some(pending) = &self.state.pending {
            highest = highest.max(pending.manifest.content_revision);
        }
        if let Some(resolution) = &self.state.last_resolution {
            highest = highest
                .max(resolution.pending.manifest.content_revision)
                .max(resolution.watermark.highest);
        }
        let evidence = Evidence {
            generation: &self.generation,
            state_hash: signed::digest(&encode(&self.state, STATE_LIMIT)?),
            target: &target,
            currents,
        };
        let digest = signed::digest(&encode(&evidence, STATE_LIMIT)?);
        Ok(Prepared {
            preview: MigrationPreview { digest },
            target,
            sequences,
            highest,
        })
    }
    pub(crate) fn migration_preview(
        &self,
        source: &mut dyn FixedCurrents,
        now: u64,
    ) -> Result<MigrationPreview> {
        Ok(self.prepare_migration(source, now)?.preview)
    }
    #[cfg(test)]
    pub(crate) fn migration_commit(
        &mut self,
        source: &mut dyn FixedCurrents,
        expected: &str,
        now: u64,
        vault: &mut dyn Vault,
    ) -> Result<()> {
        self.migration_commit_with(source, expected, now, vault, |_, _| Ok(()))
    }

    fn migration_commit_with(
        &mut self,
        source: &mut dyn FixedCurrents,
        expected: &str,
        now: u64,
        vault: &mut dyn Vault,
        before_switch: impl FnOnce(&Self, &dyn Vault) -> Result<()>,
    ) -> Result<()> {
        let prepared = self.prepare_migration(source, now)?;
        if prepared.preview.digest != expected {
            return Err(Error::Invalid("migration_stale_preview"));
        }
        let secrets = vault.load(&self.generation)?;
        self.check_signer(&secrets)?;
        let mut state = self.state.clone();
        state.schema = 2;
        state.other_targets.clear();
        state.target = Some(prepared.target.clone());
        state.draft_only_recovery = false;
        state.draft.release_id = random_id()?;
        state.draft.content_revision = prepared
            .highest
            .checked_add(1)
            .ok_or(Error::Invalid("revision_overflow"))?;
        state.history.clear();
        state.pending = None;
        state.last_resolution = None;
        state.cleanup = None;
        state.recovery_required = false;
        state.sequences = prepared.sequences;
        let hashes: std::collections::HashSet<_> = state
            .draft
            .slots
            .iter()
            .flat_map(|slot| slot.campaigns.iter())
            .map(|campaign| campaign.image.sha256.clone())
            .collect();
        for campaign in state
            .draft
            .slots
            .iter()
            .flat_map(|slot| slot.campaigns.iter())
        {
            if !state.assets.contains(&campaign.image) {
                return Err(Error::Invalid("migration_draft_asset_missing"));
            }
        }
        state.assets.retain(|asset| hashes.contains(&asset.sha256));
        let assets: Vec<BackupAsset> = state
            .assets
            .iter()
            .map(|asset| {
                Ok(BackupAsset {
                    asset: asset.clone(),
                    bytes: self.image_bytes(asset)?,
                })
            })
            .collect::<Result<_>>()?;
        self.validate_state(&state)?;
        let generation = random_id()?;
        let root = PrivateDirectory::open(&self.root)?;
        let retained = encode(
            &serde_json::json!({"schema":1,"old_generation":self.generation,"new_generation":generation,"old_state_hash":signed::digest(&root.child(&self.generation)?.read_archive_file("state.json", STATE_LIMIT)?),"target":prepared.target}),
            4096,
        )?;
        root.create_verified_file(
            &format!("retain-{}-{}.json", self.generation, generation),
            &retained,
            4096,
        )?;
        let child = root.create_child(&generation)?;
        let asset_dir = child.create_child("assets")?;
        for asset in &assets {
            asset_dir.create_verified_file(
                &asset.asset.sha256,
                &asset.bytes,
                ads::MAX_IMAGE_BYTES,
            )?;
        }
        child.create_verified_file("state.json", &encode(&state, STATE_LIMIT)?, STATE_LIMIT)?;
        vault.create(&generation, &secrets)?;
        before_switch(self, vault)?;
        if self.prepare_migration(source, now)?.preview.digest != expected {
            return Err(Error::Invalid("migration_stale_preview"));
        }
        if !self._lock.protects(&self.root)
            || source.target_identity().as_deref() != Some(prepared.target.as_str())
        {
            return Err(Error::Invalid("migration_target_changed"));
        }
        let active = encode(
            &Active {
                schema: 1,
                generation: generation.clone(),
            },
            4096,
        )?;
        let result = root.replace_file("active.json", &active);
        #[cfg(test)]
        let result =
            if result.is_ok() && ACTIVE_AFTER_COMMIT_FAULT.with(|fault| fault.replace(false)) {
                Err(Error::Invalid("migration_active_outcome_unknown"))
            } else {
                result
            };
        if result.is_ok()
            || root
                .read_archive_file("active.json", 4096)
                .is_ok_and(|bytes| bytes == active)
        {
            self.generation = generation;
            self.state = state;
        }
        result.map_err(|_| Error::Invalid("migration_active_outcome_unknown"))
    }
}

/// A distinct archive format. It is not the existing portable publisher backup.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedIndex {
    schema: u32,
    old_generation: String,
    new_generation: String,
    old_state_hash: String,
    target: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchiveFile {
    directory: String,
    name: String,
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerationArchive {
    schema: u32,
    generation: String,
    key_id: String,
    public_key: [u8; 32],
    directories: Vec<String>,
    files: Vec<ArchiveFile>,
    secrets: Secrets,
}
impl Publisher {
    /// Exports a retained generation only after checking its index and every file.
    /// Unknown entries or unreadable files reject the entire export.
    pub(crate) fn export_retained_generation(
        &self,
        generation: &str,
        passphrase: &[u8],
        vault: &dyn Vault,
    ) -> Result<Vec<u8>> {
        if !valid_generation(generation)
            || generation == self.generation
            || !self._lock.protects(&self.root)
        {
            return Err(Error::Invalid("archive_generation"));
        }
        let root = PrivateDirectory::open(&self.root)?;
        let prefix = format!("retain-{generation}-");
        let mut expected_hash = None;
        for name in root.names(4096)? {
            if !name.starts_with(&prefix) || !name.ends_with(".json") {
                continue;
            }
            let bytes = root.read_archive_file(&name, 4096)?;
            let index: RetainedIndex = signed::strict_json(&bytes, 4096).map_err(Error::Invalid)?;
            if index.schema != 1
                || index.old_generation != generation
                || !valid_generation(&index.new_generation)
                || name
                    != format!(
                        "retain-{}-{}.json",
                        index.old_generation, index.new_generation
                    )
            {
                return Err(Error::Invalid("archive_index"));
            }
            if expected_hash
                .as_ref()
                .is_some_and(|hash| hash != &index.old_state_hash)
            {
                return Err(Error::Invalid("archive_index_conflict"));
            }
            expected_hash = Some(index.old_state_hash);
        }
        let expected_hash = expected_hash.ok_or(Error::Invalid("archive_index_missing"))?;
        let state_bytes = root
            .child(generation)?
            .read_archive_file("state.json", STATE_LIMIT)?;
        if signed::digest(&state_bytes) != expected_hash {
            return Err(Error::Invalid("archive_state_changed"));
        }
        let archive = self.archive_generation(generation, vault)?;
        if archive
            .files
            .iter()
            .find(|f| f.directory.is_empty() && f.name == "state.json")
            .is_none_or(|f| f.bytes != state_bytes)
        {
            return Err(Error::Invalid("archive_state_changed"));
        }
        let plaintext = Zeroizing::new(encode(&archive, backup::LIMIT - 76)?);
        backup::seal(&plaintext, passphrase)
    }

    fn archive_generation(&self, generation: &str, vault: &dyn Vault) -> Result<GenerationArchive> {
        let root = PrivateDirectory::open(&self.root)?;
        let directory = root.child(generation)?;
        let state_bytes = directory.read_archive_file("state.json", STATE_LIMIT)?;
        let state: State =
            signed::strict_json(&state_bytes, STATE_LIMIT).map_err(Error::Invalid)?;
        cleanup::validate_batch(state.cleanup.as_ref(), generation)?;
        let mut portable = state.clone();
        portable.cleanup = None;
        self.validate_state(&portable)?;
        let secrets = vault.load(generation)?;
        self.check_signer(&secrets)?;
        let mut files = Vec::new();
        let mut directories = Vec::new();
        let mut total_bytes = 0usize;
        let mut collect = |directory: String, name: String, bytes: Vec<u8>| -> Result<()> {
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .ok_or(Error::Invalid("archive_size"))?;
            if total_bytes > (backup::LIMIT - 76) / 4 {
                return Err(Error::Invalid("archive_size"));
            }
            files.push(ArchiveFile {
                directory,
                name,
                bytes,
            });
            Ok(())
        };
        let names = directory.names(ASSET_LIMIT + 10)?;
        for name in &names {
            match name.as_str() {
                "state.json" | "cleanup-checkpoint.json" => collect(
                    String::new(),
                    name.clone(),
                    directory.read_archive_file(name, STATE_LIMIT)?,
                )?,
                "assets" | "cleanup-gc" => {
                    directories.push(name.clone());
                    let child = directory.child(name)?;
                    let child_names = child.names(ASSET_LIMIT + 1)?;
                    for file in &child_names {
                        collect(
                            name.clone(),
                            file.clone(),
                            child.read_archive_file(file, ads::MAX_IMAGE_BYTES)?,
                        )?;
                    }
                    if child.names(ASSET_LIMIT + 1)? != child_names {
                        return Err(Error::Invalid("archive_directory_changed"));
                    }
                }
                _ => return Err(Error::Invalid("archive_unknown_entry")),
            }
        }
        if directory.names(ASSET_LIMIT + 10)? != names
            || directory.read_archive_file("state.json", STATE_LIMIT)? != state_bytes
        {
            return Err(Error::Invalid("archive_state_changed"));
        }
        Ok(GenerationArchive {
            schema: 1,
            generation: generation.into(),
            key_id: self.key_id.clone(),
            public_key: self.verifying_key()?.to_bytes(),
            directories,
            files,
            secrets,
        })
    }
}

impl Publisher {
    fn validate_archive_contents(&self, archive: &GenerationArchive) -> Result<String> {
        self.check_signer(&archive.secrets)?;
        let mut dirs = std::collections::HashSet::new();
        for dir in &archive.directories {
            if (dir != "assets" && dir != "cleanup-gc") || !dirs.insert(dir.as_str()) {
                return Err(Error::Invalid("archive_directory"));
            }
        }
        if !dirs.contains("assets") || archive.files.len() > 2 * ASSET_LIMIT + 2 {
            return Err(Error::Invalid("archive_files"));
        }
        let mut files = std::collections::HashMap::new();
        for file in &archive.files {
            if file.name.is_empty()
                || file.name.len() > 100
                || file.name == "."
                || file.name == ".."
                || !file
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
                || files
                    .insert((file.directory.as_str(), file.name.as_str()), &file.bytes)
                    .is_some()
            {
                return Err(Error::Invalid("archive_file_name"));
            }
            let valid = match file.directory.as_str() {
                "" => {
                    (file.name == "state.json" || file.name == "cleanup-checkpoint.json")
                        && file.bytes.len() <= STATE_LIMIT
                }
                "assets" | "cleanup-gc" => {
                    dirs.contains(file.directory.as_str())
                        && file.bytes.len() <= ads::MAX_IMAGE_BYTES
                }
                _ => false,
            };
            if !valid {
                return Err(Error::Invalid("archive_file"));
            }
        }
        let state_bytes = files
            .get(&("", "state.json"))
            .ok_or(Error::Invalid("archive_state_missing"))?;
        let state: State = signed::strict_json(state_bytes, STATE_LIMIT).map_err(Error::Invalid)?;
        cleanup::validate_batch(state.cleanup.as_ref(), &archive.generation)?;
        if let Some(batch) = &state.cleanup {
            let checkpoint = files
                .get(&("", "cleanup-checkpoint.json"))
                .ok_or(Error::Invalid("archive_checkpoint_missing"))?;
            if signed::digest(checkpoint) != batch.checkpoint_hash {
                return Err(Error::Invalid("archive_checkpoint_changed"));
            }
        }
        let mut checked = state.clone();
        checked.cleanup = None;
        self.validate_state(&checked)?;
        for asset in &state.assets {
            let content = files
                .get(&("assets", asset.sha256.as_str()))
                .ok_or(Error::Invalid("archive_asset_missing"))?;
            ads::validate_image(content, asset).map_err(Error::Invalid)?;
        }
        Ok(signed::digest(state_bytes))
    }

    /// Imports only into a new inactive generation. The active pointer is never changed.
    pub(crate) fn import_retained_generation(
        &self,
        bytes: &[u8],
        passphrase: &[u8],
        vault: &mut dyn Vault,
    ) -> Result<()> {
        if !self._lock.protects(&self.root) {
            return Err(Error::Invalid("archive_lock"));
        }
        let plaintext = backup::open(bytes, passphrase)?;
        let archive: GenerationArchive = match signed::strict_json(&plaintext, backup::LIMIT) {
            Ok(archive) => archive,
            Err(_) => recovery::decode_archive(&plaintext)?,
        };
        if archive.schema != 1
            || !valid_generation(&archive.generation)
            || archive.generation == self.generation
            || archive.key_id != self.key_id
            || archive.public_key != self.verifying_key()?.to_bytes()
        {
            return Err(Error::Invalid("archive_scope"));
        }
        let state_hash = self.validate_archive_contents(&archive)?;
        let root = PrivateDirectory::open(&self.root)?;
        if root.names(4096)?.contains(&archive.generation) {
            return Err(Error::Invalid("archive_generation_exists"));
        }
        // Persist a non-secret recovery marker first. A partial import then
        // remains discoverable after restart, even before its Vault item exists.
        let index = RetainedIndex {
            schema: 1,
            old_generation: archive.generation.clone(),
            new_generation: self.generation.clone(),
            old_state_hash: state_hash,
            target: "imported-archive".into(),
        };
        let name = format!("retain-{}-{}.json", archive.generation, self.generation);
        root.create_verified_file(&name, &encode(&index, 4096)?, 4096)?;
        let child = root.create_child(&archive.generation)?;
        for directory in &archive.directories {
            let target = child.create_child(directory)?;
            for file in archive
                .files
                .iter()
                .filter(|file| &file.directory == directory)
            {
                target.create_verified_file(&file.name, &file.bytes, ads::MAX_IMAGE_BYTES)?;
            }
        }
        for file in archive
            .files
            .iter()
            .filter(|file| file.directory.is_empty())
        {
            child.create_verified_file(&file.name, &file.bytes, STATE_LIMIT)?;
        }
        vault.create(&archive.generation, &archive.secrets)?;
        Ok(())
    }
}

#[derive(Serialize)]
pub(crate) struct RetainedView {
    generation: String,
    state_verified: bool,
    history: Vec<HistoryRecord>,
    pending_release_id: Option<String>,
    pending_target_unknown: bool,
}
impl Publisher {
    /// Read-only discovery. Partial or damaged archives remain visible but are
    /// never presented as verified history or as resolved remote operations.
    pub(crate) fn retained_views(&self) -> Result<Vec<RetainedView>> {
        if !self._lock.protects(&self.root) {
            return Err(Error::Invalid("archive_lock"));
        }
        let root = PrivateDirectory::open(&self.root)?;
        let mut views = Vec::new();
        for name in root.names(4096)? {
            if !name.starts_with("retain-") || !name.ends_with(".json") {
                continue;
            }
            let bytes = root.read_archive_file(&name, 4096)?;
            let index: RetainedIndex = signed::strict_json(&bytes, 4096).map_err(Error::Invalid)?;
            if index.schema != 1
                || !valid_generation(&index.old_generation)
                || !valid_generation(&index.new_generation)
                || name
                    != format!(
                        "retain-{}-{}.json",
                        index.old_generation, index.new_generation
                    )
            {
                return Err(Error::Invalid("archive_index"));
            }
            let mut view = RetainedView {
                generation: index.old_generation.clone(),
                state_verified: false,
                history: Vec::new(),
                pending_release_id: None,
                pending_target_unknown: true,
            };
            if let Ok(directory) = root.child(&index.old_generation)
                && let Ok(bytes) = directory.read_archive_file("state.json", STATE_LIMIT)
                && signed::digest(&bytes) == index.old_state_hash
                && let Ok(state) = signed::strict_json::<State>(&bytes, STATE_LIMIT)
            {
                let mut checked = state.clone();
                checked.cleanup = None;
                if cleanup::validate_batch(state.cleanup.as_ref(), &index.old_generation).is_ok()
                    && self.validate_state(&checked).is_ok()
                {
                    view.state_verified = true;
                    view.history = state.history;
                    view.pending_release_id =
                        state.pending.map(|pending| pending.manifest.release_id);
                }
            }
            views.push(view);
        }
        Ok(views)
    }
}
