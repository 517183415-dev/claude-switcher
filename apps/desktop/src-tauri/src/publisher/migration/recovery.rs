//! A complete, verified encrypted archive is mandatory before changing generations.
use super::*;
use crate::publisher::local_store::LocalObjectStore;
use std::io;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    kind: String,
    length: usize,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteArchive {
    format: String,
    archive: GenerationArchive,
    trust: Option<Vec<u8>>,
    manifest: Vec<Entry>,
}
impl CompleteArchive {
    fn manifest(&self) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        let generation = &self.archive.generation;
        for directory in &self.archive.directories {
            entries.push(Entry {
                path: format!("{generation}/{directory}"),
                kind: "directory".into(),
                length: 0,
                sha256: String::new(),
            });
        }
        let mut add = |path: String, bytes: &[u8]| {
            entries.push(Entry {
                path,
                kind: "file".into(),
                length: bytes.len(),
                sha256: signed::digest(bytes),
            });
        };
        for file in &self.archive.files {
            let path = if file.directory.is_empty() {
                format!("{generation}/{}", file.name)
            } else {
                format!("{generation}/{}/{}", file.directory, file.name)
            };
            add(path, &file.bytes);
        }
        if let Some(trust) = &self.trust {
            add("trust.json".into(), trust);
        }
        let secrets = Zeroizing::new(encode(&self.archive.secrets, security::SECRET_LIMIT)?);
        add(format!("vault/{generation}"), &secrets);
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        if entries.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return Err(Error::Invalid("recovery_duplicate_path"));
        }
        Ok(entries)
    }
    fn validate(&self) -> Result<()> {
        if self.format != "complete-generation-v1" || self.manifest != self.manifest()? {
            return Err(Error::Invalid("recovery_manifest"));
        }
        if let Some(bytes) = &self.trust {
            let trust: Trust = signed::strict_json(bytes, 4096).map_err(Error::Invalid)?;
            if trust.key_id != self.archive.key_id || trust.public_key != self.archive.public_key {
                return Err(Error::Invalid("recovery_trust"));
            }
        }
        self.archive.secrets.validate()?;
        let seed: &[u8; 32] = self
            .archive
            .secrets
            .signing_seed
            .as_slice()
            .try_into()
            .map_err(|_| Error::Invalid("secret_shape"))?;
        if SigningKey::from_bytes(seed).verifying_key().to_bytes() != self.archive.public_key {
            return Err(Error::Invalid("recovery_signer"));
        }
        Ok(())
    }
}

pub(super) fn decode_archive(bytes: &[u8]) -> Result<GenerationArchive> {
    let complete: CompleteArchive =
        signed::strict_json(bytes, backup::LIMIT).map_err(Error::Invalid)?;
    complete.validate()?;
    Ok(complete.archive)
}

pub(crate) fn save_retained_archive(
    publisher: &Publisher,
    generation: &str,
    bytes: &[u8],
) -> Result<()> {
    let root = PrivateDirectory::open(&publisher.root)?;
    let backups = backup_directory(&root)?;
    backups.create_verified_file(
        &format!("retained-{generation}-{}.csbackup", random_id()?),
        bytes,
        backup::LIMIT,
    )
}

fn backup_directory(root: &PrivateDirectory) -> Result<PrivateDirectory> {
    match root.child("backups") {
        Ok(directory) => Ok(directory),
        Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            root.create_child("backups")
        }
        Err(error) => Err(error),
    }
}

fn snapshot(publisher: &Publisher, vault: &dyn Vault) -> Result<Zeroizing<Vec<u8>>> {
    if !publisher._lock.protects(&publisher.root) {
        return Err(Error::Invalid("recovery_lock"));
    }
    let root = PrivateDirectory::open(&publisher.root)?;
    let active: Active = signed::strict_json(&root.read_archive_file("active.json", 4096)?, 4096)
        .map_err(Error::Invalid)?;
    if active.schema != 1 || active.generation != publisher.generation {
        return Err(Error::Invalid("recovery_active_changed"));
    }
    let archive = publisher.archive_generation(&publisher.generation, vault)?;
    publisher.validate_archive_contents(&archive)?;
    let state = archive
        .files
        .iter()
        .find(|f| f.directory.is_empty() && f.name == "state.json")
        .ok_or(Error::Invalid("recovery_state_missing"))?;
    let disk_state: State =
        signed::strict_json(&state.bytes, STATE_LIMIT).map_err(Error::Invalid)?;
    if encode(&disk_state, STATE_LIMIT)? != encode(&publisher.state, STATE_LIMIT)? {
        return Err(Error::Invalid("recovery_state_changed"));
    }
    let trust = match root.read_archive_file("trust.json", 4096) {
        Ok(bytes) => Some(bytes),
        Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let mut complete = CompleteArchive {
        format: "complete-generation-v1".into(),
        archive,
        trust,
        manifest: Vec::new(),
    };
    complete.manifest = complete.manifest()?;
    complete.validate()?;
    Ok(Zeroizing::new(encode(&complete, backup::LIMIT - 76)?))
}

#[derive(Clone, Serialize)]
pub(crate) struct RecoveryPreview {
    pub digest: String,
    pub backup_filename: String,
    pub generation: String,
}

/// Owns both lifecycle locks across the prepare/confirm IPC boundary.
pub(crate) struct PreparedRecovery {
    publisher: Publisher,
    source: LocalObjectStore,
    archive: VerifiedBackup,
    migration_digest: String,
}
struct VerifiedBackup {
    root: PrivateDirectory,
    generation: PrivateDirectory,
    backups: PrivateDirectory,
    passphrase: Zeroizing<Vec<u8>>,
    snapshot_hash: String,
    backup_hash: String,
    preview: RecoveryPreview,
}
impl PreparedRecovery {
    pub(crate) fn prepare(
        publisher: Publisher,
        mut source: LocalObjectStore,
        passphrase: Zeroizing<Vec<u8>>,
        vault: &dyn Vault,
        now: u64,
    ) -> Result<Self> {
        let root = PrivateDirectory::open(&publisher.root)?;
        let generation = root.child(&publisher.generation)?;
        let migration_digest = publisher.migration_preview(&mut source, now)?.digest;
        let plaintext = snapshot(&publisher, vault)?;
        let snapshot_hash = signed::digest(&plaintext);
        let ciphertext = backup::seal(&plaintext, &passphrase)?;
        let backup_hash = signed::digest(&ciphertext);
        let backups = backup_directory(&root)?;
        let backup_filename = format!(
            "recovery-{}-{}.csbackup",
            publisher.generation,
            random_id()?
        );
        backups.create_verified_file(&backup_filename, &ciphertext, backup::LIMIT)?;
        let digest = signed::digest(&encode(
            &(
                &migration_digest,
                &snapshot_hash,
                &backup_hash,
                &backup_filename,
            ),
            4096,
        )?);
        let preview = RecoveryPreview {
            digest,
            backup_filename,
            generation: publisher.generation.clone(),
        };
        let mut session = Self {
            publisher,
            source,
            migration_digest,
            archive: VerifiedBackup {
                root,
                generation,
                backups,
                passphrase,
                snapshot_hash,
                backup_hash,
                preview,
            },
        };
        session.archive.verify(&session.publisher, vault)?;
        if session
            .publisher
            .migration_preview(&mut session.source, now)?
            .digest
            != session.migration_digest
        {
            return Err(Error::Invalid("migration_stale_preview"));
        }
        Ok(session)
    }
    pub(crate) fn preview(&self) -> RecoveryPreview {
        self.archive.preview.clone()
    }

    pub(crate) fn confirm(
        self,
        expected: &str,
        now: u64,
        vault: &mut dyn Vault,
    ) -> Result<RecoveryPreview> {
        if expected != self.archive.preview.digest {
            return Err(Error::Invalid("recovery_stale_confirmation"));
        }
        self.archive.verify(&self.publisher, vault)?;
        let Self {
            mut publisher,
            mut source,
            archive,
            migration_digest,
        } = self;
        publisher.migration_commit_with(
            &mut source,
            &migration_digest,
            now,
            vault,
            |publisher, vault| archive.verify(publisher, vault),
        )?;
        Ok(archive.preview)
    }
}
impl VerifiedBackup {
    fn verify(&self, publisher: &Publisher, vault: &dyn Vault) -> Result<()> {
        self.root.check()?;
        self.generation.check()?;
        self.backups.check()?;
        let bytes = self
            .backups
            .read_archive_file(&self.preview.backup_filename, backup::LIMIT)?;
        if signed::digest(&bytes) != self.backup_hash {
            return Err(Error::Invalid("recovery_backup_changed"));
        }
        let plaintext = backup::open(&bytes, &self.passphrase)?;
        decode_archive(&plaintext)?;
        if signed::digest(&plaintext) != self.snapshot_hash
            || signed::digest(&snapshot(publisher, vault)?) != self.snapshot_hash
        {
            return Err(Error::Invalid("recovery_source_changed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
