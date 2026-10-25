//! Personal-only local editor, recoverable publication and encrypted backup.
pub mod backup;
pub mod local_store;
pub mod protocol;
pub mod security;

use chacha20poly1305::aead::{OsRng, rand_core::RngCore};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use protocol::{ObjectStore, Pending, Phase, StoreError};
use security::{Secrets, Vault};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use switcher_core::{ads, signed, storage};
use switcher_domain::{
    AdSlot,
    ads::{AdChannel, AdManifest, ImageAsset, SlotContent},
};
use zeroize::Zeroizing;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub enum Error {
    Invalid(&'static str),
    Io(std::io::Error),
    SecretUnavailable,
    Random,
    MissingSource,
    MissingSigningKey,
    PendingOperation,
    RecoveryReadRequired,
    Store(StoreError),
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "发布数据无效：{reason}"),
            Self::Io(_) => f.write_str("本地发布数据读写失败"),
            Self::SecretUnavailable => f.write_str("Keychain 凭据不可用或访问被取消"),
            Self::Random => f.write_str("安全随机数不可用"),
            Self::MissingSigningKey => {
                f.write_str("尚未配置广告签名公钥，请先导入或明确生成广告密钥")
            }
            Self::MissingSource => f.write_str("尚未配置广告托管源，未执行远端发布"),
            Self::PendingOperation => f.write_str("请先回查未完成发布，沿用已有 release_id"),
            Self::RecoveryReadRequired => f.write_str("恢复备份后，必须先回读正式最高内容序号"),
            Self::Store(StoreError::Conflict) => {
                f.write_str("内容已被其他发布者更改，请重新读取并合并")
            }
            Self::Store(StoreError::OutcomeUnknown) => {
                f.write_str("发布结果未知，将先回查已有 release_id")
            }
            Self::Store(StoreError::Unavailable) => f.write_str("托管源访问或公开回读失败"),
            Self::Store(StoreError::Permission) => f.write_str("托管源访问被拒绝"),
        }
    }
}
impl std::error::Error for Error {}

const STATE_LIMIT: usize = 16 * 1024 * 1024;
const HISTORY_LIMIT: usize = 256;
const ASSET_LIMIT: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryRecord {
    pub manifest: AdManifest,
    pub published_at: u64,
    pub payload_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u32,
    draft: AdManifest,
    history: Vec<HistoryRecord>,
    assets: Vec<ImageAsset>,
    sequences: [signed::Sequence; 2],
    pending: Option<Pending>,
    recovery_required: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Trust {
    key_id: String,
    public_key: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Active {
    schema: u32,
    generation: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupAsset {
    asset: ImageAsset,
    bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    key_id: String,
    public_key: [u8; 32],
    state: State,
    assets: Vec<BackupAsset>,
    secrets: Secrets,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub phase: Option<Phase>,
    pub pending_release_id: Option<String>,
    pub recovery_required: bool,
    pub history: Vec<HistoryRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication {
    pub release_id: String,
    pub revision: u64,
    pub channel: AdChannel,
}

/// Keep one instance behind the desktop's mutex. The root lock excludes other
/// Personal processes. No real source or Keychain write is implicit in `open`.
pub struct Publisher {
    root: PathBuf,
    generation: String,
    state: State,
    key_id: String,
    public_key: Option<VerifyingKey>,
    _lock: storage::LifecycleLock,
}

fn channel_index(channel: AdChannel) -> usize {
    if channel == AdChannel::Preview { 0 } else { 1 }
}
fn random_id() -> Result<String> {
    let mut bytes = [0; 16];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| Error::Random)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
fn valid_generation(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn encode<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Invalid("local_json"))?;
    if bytes.len() > limit {
        return Err(Error::Invalid("local_size"));
    }
    Ok(bytes)
}
fn read_private(path: &Path, limit: usize) -> Result<Vec<u8>> {
    storage::validate_no_symlinks(path.parent().ok_or(Error::Invalid("local_path"))?)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != fs::metadata(path.parent().ok_or(Error::Invalid("local_path"))?)?.uid()
        || metadata.mode() & 0o077 != 0
        || metadata.len() > limit as u64
    {
        return Err(Error::Invalid("unsafe_local_file"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::Invalid("local_size"));
    }
    Ok(bytes)
}
impl Publisher {
    pub fn open(
        root: &Path,
        key_id: &str,
        public_key: Option<VerifyingKey>,
        now: u64,
    ) -> Result<Self> {
        storage::create_private_directory(root)?;
        let lock = storage::LifecycleLock::try_acquire(root)?;
        let mut public_key = public_key;
        match read_private(&root.join("trust.json"), 4096) {
            Ok(bytes) => {
                let trust: Trust = signed::strict_json(&bytes, 4096).map_err(Error::Invalid)?;
                if trust.key_id != key_id {
                    return Err(Error::Invalid("trust_key_id"));
                }
                let stored = VerifyingKey::from_bytes(&trust.public_key)
                    .map_err(|_| Error::Invalid("trust_public_key"))?;
                if public_key.is_some_and(|key| key != stored) {
                    return Err(Error::Invalid("trust_public_key_mismatch"));
                }
                public_key = Some(stored);
            }
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let active_path = root.join("active.json");
        let (generation, state) = match read_private(&active_path, 4096) {
            Ok(bytes) => {
                let active: Active = signed::strict_json(&bytes, 4096).map_err(Error::Invalid)?;
                if active.schema != 1 || !valid_generation(&active.generation) {
                    return Err(Error::Invalid("active_generation"));
                }
                let state = signed::strict_json(
                    &read_private(
                        &root.join(&active.generation).join("state.json"),
                        STATE_LIMIT,
                    )?,
                    STATE_LIMIT,
                )
                .map_err(Error::Invalid)?;
                (active.generation, state)
            }
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                let generation = random_id()?;
                let draft = AdManifest {
                    schema_version: 1,
                    channel: AdChannel::Preview,
                    content_revision: 1,
                    key_id: key_id.into(),
                    issued_at: now,
                    expires_at: now
                        .checked_add(ads::MAX_MANIFEST_LIFETIME)
                        .ok_or(Error::Invalid("draft_time"))?,
                    release_id: random_id()?,
                    slots: AdSlot::ALL
                        .into_iter()
                        .map(|slot| SlotContent {
                            slot,
                            enabled: true,
                            campaigns: vec![],
                        })
                        .collect(),
                };
                ads::validate_manifest(&draft, draft.channel, key_id, now)
                    .map_err(Error::Invalid)?;
                let state = State {
                    schema: 1,
                    draft,
                    history: vec![],
                    assets: vec![],
                    sequences: Default::default(),
                    pending: None,
                    recovery_required: false,
                };
                Self::write_generation(root, &generation, &state, &[])?;
                storage::atomic_write(
                    &active_path,
                    &encode(
                        &Active {
                            schema: 1,
                            generation: generation.clone(),
                        },
                        4096,
                    )?,
                )?;
                (generation, state)
            }
            Err(error) => return Err(error),
        };
        let publisher = Self {
            root: root.into(),
            generation,
            state,
            key_id: key_id.into(),
            public_key,
            _lock: lock,
        };
        publisher.validate_state(&publisher.state)?;
        Ok(publisher)
    }

    fn directory(&self) -> PathBuf {
        self.root.join(&self.generation)
    }
    fn persist(&mut self, state: State) -> Result<()> {
        self.validate_state(&state)?;
        storage::atomic_write(
            &self.directory().join("state.json"),
            &encode(&state, STATE_LIMIT)?,
        )?;
        self.state = state;
        Ok(())
    }
    fn validate_state(&self, state: &State) -> Result<()> {
        if state.schema != 1
            || state.history.len() > HISTORY_LIMIT
            || state.assets.len() > ASSET_LIMIT
        {
            return Err(Error::Invalid("publisher_state"));
        }
        ads::validate_manifest(
            &state.draft,
            state.draft.channel,
            &self.key_id,
            state.draft.issued_at,
        )
        .map_err(Error::Invalid)?;
        let mut hashes = std::collections::HashSet::new();
        for asset in &state.assets {
            ads::validate_asset(asset).map_err(Error::Invalid)?;
            if !hashes.insert(&asset.sha256) {
                return Err(Error::Invalid("asset_duplicate"));
            }
        }
        let mut releases = std::collections::HashSet::new();
        for record in &state.history {
            ads::validate_manifest(
                &record.manifest,
                record.manifest.channel,
                &self.key_id,
                record.manifest.issued_at,
            )
            .map_err(Error::Invalid)?;
            if !releases.insert((
                &record.manifest.release_id,
                channel_index(record.manifest.channel),
            )) || record.payload_hash
                != signed::digest(&encode(&record.manifest, ads::MAX_MANIFEST_BYTES)?)
            {
                return Err(Error::Invalid("history_record"));
            }
        }
        if let Some(pending) = &state.pending {
            let manifest = if let Some(key) = &self.public_key {
                ads::verify_manifest(
                    &pending.wire,
                    key,
                    pending.manifest.channel,
                    &self.key_id,
                    pending.manifest.issued_at,
                    true,
                )
                .map_err(Error::Invalid)?
                .0
            } else {
                let envelope: signed::Envelope =
                    signed::strict_json(&pending.wire, ads::MAX_MANIFEST_BYTES)
                        .map_err(Error::Invalid)?;
                let manifest: AdManifest =
                    signed::strict_json(envelope.payload.as_bytes(), ads::MAX_MANIFEST_BYTES)
                        .map_err(Error::Invalid)?;
                ads::validate_manifest(
                    &manifest,
                    pending.manifest.channel,
                    &self.key_id,
                    manifest.issued_at,
                )
                .map_err(Error::Invalid)?;
                manifest
            };
            if manifest != pending.manifest {
                return Err(Error::Invalid("pending_manifest"));
            }
            if pending.expected_etag.as_ref().is_some_and(|etag| {
                etag.is_empty() || etag.len() > 1024 || etag.chars().any(char::is_control)
            }) {
                return Err(Error::Invalid("etag"));
            }
        }
        Ok(())
    }
    pub fn draft_read(&self) -> AdManifest {
        self.state.draft.clone()
    }
    pub fn draft_save(&mut self, mut manifest: AdManifest, now: u64) -> Result<AdManifest> {
        if self
            .state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.phase != Phase::Conflict)
        {
            return Err(Error::PendingOperation);
        }
        let next = self
            .state
            .sequences
            .iter()
            .map(|sequence| sequence.highest)
            .chain([self.state.draft.content_revision])
            .max()
            .unwrap_or_default()
            .checked_add(1)
            .ok_or(Error::Invalid("revision_overflow"))?;
        manifest.content_revision = next;
        manifest.release_id = random_id()?;
        manifest.issued_at = now;
        manifest.expires_at = now
            .checked_add(ads::MAX_MANIFEST_LIFETIME)
            .ok_or(Error::Invalid("draft_time"))?;
        ads::validate_manifest(&manifest, manifest.channel, &self.key_id, now)
            .map_err(Error::Invalid)?;
        let mut state = self.state.clone();
        state.draft = manifest.clone();
        state.pending = None;
        self.persist(state)?;
        Ok(manifest)
    }
    #[cfg(test)]
    fn draft_save_bytes(&mut self, bytes: &[u8], now: u64) -> Result<AdManifest> {
        self.draft_save(
            signed::strict_json(bytes, ads::MAX_MANIFEST_BYTES).map_err(Error::Invalid)?,
            now,
        )
    }
    pub fn pause(&mut self, slot: Option<AdSlot>, now: u64) -> Result<AdManifest> {
        let mut draft = self.draft_read();
        for content in &mut draft.slots {
            if slot.is_none_or(|slot| content.slot == slot) {
                content.enabled = false;
            }
        }
        self.draft_save(draft, now)
    }
    pub fn restore_history(&mut self, release_id: &str, now: u64) -> Result<AdManifest> {
        let manifest = self
            .state
            .history
            .iter()
            .rev()
            .find(|record| record.manifest.release_id == release_id)
            .ok_or(Error::Invalid("history_missing"))?
            .manifest
            .clone();
        self.draft_save(manifest, now)
    }
    pub fn import_image(&mut self, bytes: &[u8], mime: &str) -> Result<ImageAsset> {
        let (bytes, asset) = ads::normalize_image(bytes, mime).map_err(Error::Invalid)?;
        if let Some(existing) = self
            .state
            .assets
            .iter()
            .find(|existing| existing.sha256 == asset.sha256)
        {
            if existing != &asset {
                return Err(Error::Invalid("asset_conflict"));
            }
            return Ok(existing.clone());
        }
        if self.state.assets.len() >= ASSET_LIMIT {
            return Err(Error::Invalid("asset_capacity_no_history_pruned"));
        }
        storage::atomic_write(&self.directory().join("assets").join(&asset.sha256), &bytes)?;
        let mut state = self.state.clone();
        state.assets.push(asset.clone());
        self.persist(state)?;
        Ok(asset)
    }
    fn image_bytes(&self, asset: &ImageAsset) -> Result<Vec<u8>> {
        ads::validate_asset(asset).map_err(Error::Invalid)?;
        if !self.state.assets.contains(asset) {
            return Err(Error::Invalid("asset_missing"));
        }
        let bytes = read_private(
            &self.directory().join("assets").join(&asset.sha256),
            ads::MAX_IMAGE_BYTES,
        )?;
        ads::validate_image(&bytes, asset).map_err(Error::Invalid)?;
        Ok(bytes)
    }
    pub fn image_preview(&self, sha256: &str) -> Result<String> {
        let asset = self
            .state
            .assets
            .iter()
            .find(|asset| asset.sha256 == sha256)
            .ok_or(Error::Invalid("asset_missing"))?;
        let bytes = self.image_bytes(asset)?;
        use base64::Engine;
        let result = format!(
            "data:{};base64,{}",
            asset.mime,
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        Ok(result)
    }
    pub fn status(&self) -> Status {
        Status {
            phase: self.state.pending.as_ref().map(|pending| pending.phase),
            pending_release_id: self
                .state
                .pending
                .as_ref()
                .map(|pending| pending.manifest.release_id.clone()),
            recovery_required: self.state.recovery_required,
            history: self.state.history.clone(),
        }
    }
    pub fn credentials_save(&mut self, vault: &mut dyn Vault, secrets: &Secrets) -> Result<()> {
        let signer = self.check_signer(secrets)?;
        vault.store(&self.generation, secrets)?;
        storage::atomic_write(
            &self.root.join("trust.json"),
            &encode(
                &Trust {
                    key_id: self.key_id.clone(),
                    public_key: signer.verifying_key().to_bytes(),
                },
                4096,
            )?,
        )?;
        self.public_key = Some(signer.verifying_key());
        Ok(())
    }
    /// Call only for an explicit operator action. Opening drafts never generates a key.
    pub fn generate_signing_key(
        &mut self,
        vault: &mut dyn Vault,
        preview_credential: Vec<u8>,
        production_credential: Vec<u8>,
    ) -> Result<[u8; 32]> {
        if self.public_key.is_some() {
            return Err(Error::Invalid("signing_key_already_configured"));
        }
        let mut secrets = Secrets {
            signing_seed: vec![0; 32],
            preview_credential,
            production_credential,
        };
        OsRng
            .try_fill_bytes(&mut secrets.signing_seed)
            .map_err(|_| Error::Random)?;
        self.credentials_save(vault, &secrets)?;
        Ok(self.verifying_key()?.to_bytes())
    }
    pub fn configured_public_key(&self) -> Option<[u8; 32]> {
        self.public_key.map(|key| key.to_bytes())
    }
    fn verifying_key(&self) -> Result<&VerifyingKey> {
        self.public_key.as_ref().ok_or(Error::MissingSigningKey)
    }
    fn check_signer(&self, secrets: &Secrets) -> Result<SigningKey> {
        secrets.validate()?;
        let seed: &[u8; 32] = secrets
            .signing_seed
            .as_slice()
            .try_into()
            .map_err(|_| Error::Invalid("signing_seed"))?;
        let signer = SigningKey::from_bytes(seed);
        if self
            .public_key
            .is_some_and(|key| signer.verifying_key() != key)
        {
            return Err(Error::Invalid("signing_public_key_mismatch"));
        }
        Ok(signer)
    }

    pub fn publish(
        &mut self,
        channel: AdChannel,
        now: u64,
        source: Option<&mut dyn ObjectStore>,
        vault: &dyn Vault,
    ) -> Result<Publication> {
        let source = source.ok_or(Error::MissingSource)?;
        self.publish_with(source, vault, channel, now)
    }
    fn read_remote(
        &mut self,
        source: &mut dyn ObjectStore,
        now: u64,
    ) -> Result<Option<protocol::Object>> {
        let current = source.current()?;
        let channel = source.channel();
        let mut state = self.state.clone();
        if let Some(object) = &current {
            if object.etag.is_empty()
                || object.etag.len() > 1024
                || object.etag.chars().any(char::is_control)
            {
                return Err(Error::Invalid("etag"));
            }
            let (manifest, hash): (AdManifest, String) = signed::verify(
                &object.bytes,
                self.verifying_key()?,
                ads::MAX_MANIFEST_BYTES,
            )
            .map_err(Error::Invalid)?;
            // Expired current still authenticates its replay watermark, never display.
            ads::validate_manifest(&manifest, channel, &self.key_id, manifest.issued_at)
                .map_err(Error::Invalid)?;
            if manifest.issued_at > now {
                return Err(Error::Invalid("remote_clock"));
            }
            state.sequences[channel_index(channel)]
                .remember(manifest.content_revision, hash)
                .map_err(Error::Invalid)?;
        } else if state.sequences[channel_index(channel)].highest != 0 {
            return Err(Error::Invalid(
                "remote_current_missing_after_authenticated_release",
            ));
        }
        if channel == AdChannel::Production {
            state.recovery_required = false;
        }
        self.persist(state)?;
        Ok(current)
    }
    // Production publishing performs this read through publish_with. The test
    // entry isolates recovery's watermark gate without publishing a new draft.
    #[cfg(test)]
    fn reconcile_production(&mut self, source: &mut dyn ObjectStore, now: u64) -> Result<()> {
        protocol::require_scope(source, AdChannel::Production)?;
        self.read_remote(source, now)?;
        Ok(())
    }
    pub fn publish_with(
        &mut self,
        source: &mut dyn ObjectStore,
        vault: &dyn Vault,
        channel: AdChannel,
        now: u64,
    ) -> Result<Publication> {
        protocol::require_scope(source, channel)?;
        self.verifying_key()?;
        self.validate_state(&self.state)?;
        if self.state.recovery_required && channel != AdChannel::Production {
            return Err(Error::RecoveryReadRequired);
        }
        if self.state.recovery_required {
            self.read_remote(source, now)?;
        }
        if self.state.pending.is_none() {
            if self.state.history.len() >= HISTORY_LIMIT {
                return Err(Error::Invalid("history_capacity_no_history_pruned"));
            }
            let current = self.read_remote(source, now)?;
            if let Some(object) = &current {
                let (manifest, _): (AdManifest, String) = signed::verify(
                    &object.bytes,
                    self.verifying_key()?,
                    ads::MAX_MANIFEST_BYTES,
                )
                .map_err(Error::Invalid)?;
                if manifest.expires_at > now
                    && manifest == self.state.draft
                    && self
                        .state
                        .history
                        .iter()
                        .any(|record| record.manifest == manifest)
                {
                    let mut state = self.state.clone();
                    state.pending = Some(Pending {
                        manifest,
                        wire: object.bytes.clone(),
                        expected_etag: Some(object.etag.clone()),
                        phase: Phase::ReadbackFailed,
                    });
                    self.persist(state)?;
                    return self.finish_readback(source, now);
                }
            }
            let secrets = vault.load(&self.generation)?;
            let signer = self.check_signer(&secrets)?;
            let credential = if channel == AdChannel::Preview {
                &secrets.preview_credential
            } else {
                &secrets.production_credential
            };
            if source.requires_credentials() && credential.is_empty() {
                return Err(Error::SecretUnavailable);
            }
            let mut manifest = self.draft_read();
            manifest.channel = channel;
            if self.state.history.iter().any(|record| {
                record.manifest.release_id == manifest.release_id
                    && record.manifest.channel == channel
            }) {
                manifest.release_id = random_id()?;
            }
            manifest.content_revision = manifest.content_revision.max(
                self.state.sequences[channel_index(channel)]
                    .highest
                    .checked_add(1)
                    .ok_or(Error::Invalid("revision_overflow"))?,
            );
            manifest.issued_at = now;
            manifest.expires_at = now
                .checked_add(ads::MAX_MANIFEST_LIFETIME)
                .ok_or(Error::Invalid("draft_time"))?;
            ads::validate_manifest(&manifest, channel, &self.key_id, now)
                .map_err(Error::Invalid)?;
            for campaign in manifest.slots.iter().flat_map(|slot| &slot.campaigns) {
                self.image_bytes(&campaign.image)?;
            }
            let payload = String::from_utf8(encode(&manifest, ads::MAX_MANIFEST_BYTES)?)
                .map_err(|_| Error::Invalid("manifest_utf8"))?;
            let signature = signer.sign(payload.as_bytes()).to_bytes().to_vec();
            let wire = encode(
                &signed::Envelope { payload, signature },
                ads::MAX_MANIFEST_BYTES,
            )?;
            let mut state = self.state.clone();
            state.pending = Some(Pending {
                manifest,
                wire,
                expected_etag: current.map(|object| object.etag),
                phase: Phase::Prepared,
            });
            self.persist(state)?;
        }
        let pending = self
            .state
            .pending
            .clone()
            .ok_or(Error::Invalid("pending_missing"))?;
        protocol::require_scope(source, pending.manifest.channel)?;
        if pending.manifest.channel != channel {
            return Err(Error::PendingOperation);
        }
        if pending.phase == Phase::Conflict {
            return Err(Error::Store(StoreError::Conflict));
        }
        let public_current =
            source.public_read(&protocol::current_key(channel), ads::MAX_MANIFEST_BYTES)?;
        if protocol::same_object(&public_current, &pending.wire) {
            return self.finish_readback(source, now);
        }
        if matches!(pending.phase, Phase::CommitUnknown | Phase::ReadbackFailed) {
            // Resolve the ambiguous authenticated commit before another write.
            let current = source.current()?;
            if protocol::same_object(&current, &pending.wire) {
                return self.finish_readback(source, now);
            }
            if current.as_ref().map(|object| &object.etag) != pending.expected_etag.as_ref() {
                self.set_phase(Phase::Conflict)?;
                return Err(Error::Store(StoreError::Conflict));
            }
            let version = source.public_read(
                &protocol::version_key(&pending.manifest),
                ads::MAX_MANIFEST_BYTES,
            )?;
            if !protocol::same_object(&version, &pending.wire) {
                return Err(Error::Store(StoreError::OutcomeUnknown));
            }
        }
        self.set_phase(Phase::Uploading)?;
        for campaign in pending
            .manifest
            .slots
            .iter()
            .flat_map(|slot| &slot.campaigns)
        {
            let bytes = self.image_bytes(&campaign.image)?;
            let key = protocol::asset_key(channel, &campaign.image);
            source.put_immutable(&key, &bytes)?;
        }
        source.put_immutable(&protocol::version_key(&pending.manifest), &pending.wire)?;
        // Persist the uncertain state BEFORE sending the CAS request.
        self.set_phase(Phase::CommitUnknown)?;
        match source.commit_current(pending.expected_etag.as_deref(), &pending.wire) {
            Ok(()) => self.finish_readback(source, now),
            Err(StoreError::Conflict) => {
                self.set_phase(Phase::Conflict)?;
                Err(Error::Store(StoreError::Conflict))
            }
            Err(error) => Err(Error::Store(error)),
        }
    }
    fn set_phase(&mut self, phase: Phase) -> Result<()> {
        let mut state = self.state.clone();
        state
            .pending
            .as_mut()
            .ok_or(Error::Invalid("pending_missing"))?
            .phase = phase;
        self.persist(state)
    }
    fn finish_readback(&mut self, source: &mut dyn ObjectStore, now: u64) -> Result<Publication> {
        let result = self.verify_public_readback(source, now);
        if result.is_err() {
            self.set_phase(Phase::ReadbackFailed)?;
        }
        result
    }
    fn verify_public_readback(
        &mut self,
        source: &mut dyn ObjectStore,
        now: u64,
    ) -> Result<Publication> {
        let pending = self
            .state
            .pending
            .as_ref()
            .ok_or(Error::Invalid("pending_missing"))?;
        let current = source.public_read(
            &protocol::current_key(pending.manifest.channel),
            ads::MAX_MANIFEST_BYTES,
        )?;
        let current = current.ok_or(Error::Store(StoreError::Unavailable))?;
        if current.bytes != pending.wire {
            return Err(Error::Invalid("public_current_readback"));
        }
        let (manifest, hash) = ads::verify_manifest(
            &current.bytes,
            self.verifying_key()?,
            pending.manifest.channel,
            &self.key_id,
            now,
            true,
        )
        .map_err(Error::Invalid)?;
        if manifest != pending.manifest {
            return Err(Error::Invalid("readback_release"));
        }
        let version = source
            .public_read(&protocol::version_key(&manifest), ads::MAX_MANIFEST_BYTES)?
            .ok_or(Error::Store(StoreError::Unavailable))?;
        if version.bytes != pending.wire {
            return Err(Error::Invalid("public_version_readback"));
        }
        for campaign in manifest.slots.iter().flat_map(|slot| &slot.campaigns) {
            let object = source
                .public_read(
                    &protocol::asset_key(manifest.channel, &campaign.image),
                    ads::MAX_IMAGE_BYTES,
                )?
                .ok_or(Error::Store(StoreError::Unavailable))?;
            ads::validate_image(&object.bytes, &campaign.image).map_err(Error::Invalid)?;
        }
        let outcome = Publication {
            release_id: manifest.release_id.clone(),
            revision: manifest.content_revision,
            channel: manifest.channel,
        };
        let mut state = self.state.clone();
        state.sequences[channel_index(manifest.channel)]
            .remember(manifest.content_revision, hash.clone())
            .map_err(Error::Invalid)?;
        if !state
            .history
            .iter()
            .any(|record| record.manifest == manifest)
        {
            state.history.push(HistoryRecord {
                manifest: manifest.clone(),
                published_at: now,
                payload_hash: hash,
            });
        }
        state.draft = manifest;
        state.pending = None;
        self.persist(state)?;
        Ok(outcome)
    }

    pub fn export_backup(&self, passphrase: &[u8], vault: &dyn Vault) -> Result<Vec<u8>> {
        let secrets = vault.load(&self.generation)?;
        self.check_signer(&secrets)?;
        let assets = self
            .state
            .assets
            .iter()
            .map(|asset| {
                self.image_bytes(asset).map(|bytes| BackupAsset {
                    asset: asset.clone(),
                    bytes,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let snapshot = Snapshot {
            schema: 1,
            key_id: self.key_id.clone(),
            public_key: self.verifying_key()?.to_bytes(),
            state: self.state.clone(),
            assets,
            secrets,
        };
        let plaintext = Zeroizing::new(encode(&snapshot, backup::LIMIT - 76)?);
        backup::seal(&plaintext, passphrase)
    }
    pub fn restore_backup(
        &mut self,
        bytes: &[u8],
        passphrase: &[u8],
        vault: &mut dyn Vault,
    ) -> Result<()> {
        if self
            .state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.phase != Phase::Conflict)
        {
            return Err(Error::PendingOperation);
        }
        let plaintext = backup::open(bytes, passphrase)?;
        let mut snapshot: Snapshot =
            signed::strict_json(&plaintext, backup::LIMIT).map_err(Error::Invalid)?;
        if snapshot.schema != 1
            || snapshot.key_id != self.key_id
            || self
                .public_key
                .is_some_and(|key| snapshot.public_key != key.to_bytes())
        {
            return Err(Error::Invalid("backup_scope"));
        }
        let restored_key = VerifyingKey::from_bytes(&snapshot.public_key)
            .map_err(|_| Error::Invalid("backup_public_key"))?;
        if self.check_signer(&snapshot.secrets)?.verifying_key() != restored_key {
            return Err(Error::Invalid("backup_secret_key_mismatch"));
        }
        if let Some(pending) = &snapshot.state.pending {
            ads::verify_manifest(
                &pending.wire,
                &restored_key,
                pending.manifest.channel,
                &self.key_id,
                pending.manifest.issued_at,
                true,
            )
            .map_err(Error::Invalid)?;
        }
        self.validate_state(&snapshot.state)?;
        if snapshot.assets.len() != snapshot.state.assets.len() {
            return Err(Error::Invalid("backup_asset_index"));
        }
        let mut hashes = std::collections::HashSet::new();
        for asset in &snapshot.assets {
            if !snapshot.state.assets.contains(&asset.asset) || !hashes.insert(&asset.asset.sha256)
            {
                return Err(Error::Invalid("backup_asset_index"));
            }
            ads::validate_image(&asset.bytes, &asset.asset).map_err(Error::Invalid)?;
        }
        // Keep the local replay watermark when restoring an older backup.
        for index in 0..2 {
            let local = &self.state.sequences[index];
            if local.highest >= snapshot.state.sequences[index].highest {
                snapshot.state.sequences[index] = local.clone();
            }
        }
        snapshot.state.recovery_required = true;
        if snapshot.state.pending.is_none() {
            // A previous installation may already have used this draft's ID.
            snapshot.state.draft.release_id = random_id()?;
            snapshot.state.draft.content_revision = snapshot
                .state
                .sequences
                .iter()
                .map(|sequence| sequence.highest)
                .chain([snapshot.state.draft.content_revision])
                .max()
                .unwrap_or_default()
                .checked_add(1)
                .ok_or(Error::Invalid("revision_overflow"))?;
        }
        self.validate_state(&snapshot.state)?;
        let generation = random_id()?;
        Self::write_generation(&self.root, &generation, &snapshot.state, &snapshot.assets)?;
        // Both old active state and old Keychain item remain valid on any failure.
        // New files and the new secret item become reachable in one pointer rename.
        vault.store(&generation, &snapshot.secrets)?;
        if self.public_key.is_none() {
            storage::atomic_write(
                &self.root.join("trust.json"),
                &encode(
                    &Trust {
                        key_id: self.key_id.clone(),
                        public_key: restored_key.to_bytes(),
                    },
                    4096,
                )?,
            )?;
            self.public_key = Some(restored_key);
        }
        let active = encode(
            &Active {
                schema: 1,
                generation: generation.clone(),
            },
            4096,
        )?;
        let committed = storage::atomic_write(&self.root.join("active.json"), &active);
        // Directory sync may fail after the rename commit point. Match the
        // on-disk generation in memory while preserving the durability error.
        if committed.is_ok()
            || read_private(&self.root.join("active.json"), 4096).is_ok_and(|bytes| bytes == active)
        {
            self.generation = generation;
            self.state = snapshot.state;
        }
        committed.map_err(Error::Io)
    }
    fn write_generation(
        root: &Path,
        generation: &str,
        state: &State,
        assets: &[BackupAsset],
    ) -> Result<()> {
        let directory = root.join(generation);
        storage::create_private_directory(&directory)?;
        let asset_directory = directory.join("assets");
        storage::create_private_directory(&asset_directory)?;
        for asset in assets {
            storage::atomic_write(&asset_directory.join(&asset.asset.sha256), &asset.bytes)?;
        }
        storage::atomic_write(&directory.join("state.json"), &encode(state, STATE_LIMIT)?)?;
        storage::sync_directory(root)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
