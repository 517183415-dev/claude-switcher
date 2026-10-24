//! The only configured publication adapter: private files on this user's Mac.
use super::{
    Error, Result,
    protocol::{self, Object, ObjectStore, StoreError},
};
use std::{
    ffi::CString,
    fs, io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use switcher_core::{
    ad_runtime::local::{LocalTrust, directory, validate_directory},
    ads, signed, storage,
};
use switcher_domain::ads::AdChannel;

pub struct LocalObjectStore {
    root: PathBuf,
    channel: AdChannel,
    lock: storage::LifecycleLock,
}
fn failure(error: io::Error) -> StoreError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        StoreError::Permission
    } else {
        StoreError::Unavailable
    }
}
impl LocalObjectStore {
    /// The application root comes from DesktopState, never an IPC argument.
    pub fn open(application_root: &Path, channel: AdChannel, trust: &LocalTrust) -> Result<Self> {
        trust.validate().map_err(Error::Invalid)?;
        let root = directory(application_root);
        storage::create_private_directory(&root)?;
        let lock = storage::LifecycleLock::try_acquire(&root)?;
        for environment in ["preview", "production"] {
            let parent = root.join(environment);
            storage::create_private_directory(&parent)?;
            storage::create_private_directory(&parent.join("assets"))?;
            storage::create_private_directory(&parent.join("versions"))?;
        }
        let path = root.join("trust.json");
        match storage::read_private_bytes(&path, 4096) {
            Ok(bytes) => {
                let previous: LocalTrust =
                    signed::strict_json(&bytes, 4096).map_err(Error::Invalid)?;
                if &previous != trust {
                    return Err(Error::Invalid("local_trust_key_changed"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                storage::atomic_write(
                    &path,
                    &serde_json::to_vec(trust).map_err(|_| Error::Invalid("local_trust_json"))?,
                )?;
            }
            Err(error) => return Err(Error::Io(error)),
        }
        Ok(Self {
            root,
            channel,
            lock,
        })
    }
    fn path(&self, key: &str) -> std::result::Result<PathBuf, StoreError> {
        if !self.lock.protects(&self.root) {
            return Err(StoreError::Permission);
        }
        validate_directory(&self.root).map_err(|_| StoreError::Permission)?;
        let prefix = format!("{}/", protocol::prefix(self.channel));
        let suffix = key.strip_prefix(&prefix).ok_or(StoreError::Permission)?;
        let valid = suffix == "current.json"
            || suffix.strip_prefix("assets/").is_some_and(|name| {
                name.rsplit_once('.').is_some_and(|(hash, extension)| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                        && matches!(extension, "png" | "jpg")
                })
            })
            || suffix
                .strip_prefix("versions/")
                .and_then(|name| name.strip_suffix(".json"))
                .is_some_and(|id| {
                    !id.is_empty()
                        && id.len() <= 128
                        && id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                });
        if !valid {
            return Err(StoreError::Permission);
        }
        let path = self.root.join(key);
        validate_directory(&self.root.join(protocol::prefix(self.channel)))
            .map_err(|_| StoreError::Permission)?;
        validate_directory(path.parent().ok_or(StoreError::Permission)?)
            .map_err(|_| StoreError::Permission)?;
        Ok(path)
    }
    fn read(&self, key: &str, limit: usize) -> std::result::Result<Option<Object>, StoreError> {
        if limit > ads::MAX_IMAGE_BYTES {
            return Err(StoreError::Permission);
        }
        let path = self.path(key)?;
        match storage::read_private_bytes(&path, limit) {
            Ok(bytes) => Ok(Some(Object {
                etag: signed::digest(&bytes),
                bytes,
            })),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(failure(error)),
        }
    }
}
impl ObjectStore for LocalObjectStore {
    fn channel(&self) -> AdChannel {
        self.channel
    }
    fn requires_credentials(&self) -> bool {
        false
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        self.read(
            &protocol::current_key(self.channel),
            ads::MAX_MANIFEST_BYTES,
        )
    }
    fn public_read(
        &mut self,
        key: &str,
        limit: usize,
    ) -> std::result::Result<Option<Object>, StoreError> {
        self.read(key, limit)
    }
    fn put_immutable(&mut self, key: &str, bytes: &[u8]) -> std::result::Result<(), StoreError> {
        if bytes.len() > ads::MAX_IMAGE_BYTES || key == protocol::current_key(self.channel) {
            return Err(StoreError::Permission);
        }
        let target = self.path(key)?;
        if let Some(existing) = self.read(key, ads::MAX_IMAGE_BYTES)? {
            return if existing.bytes == bytes {
                storage::sync_directory(target.parent().ok_or(StoreError::Permission)?)
                    .map_err(failure)
            } else {
                Err(StoreError::Conflict)
            };
        }
        let temporary = target.parent().ok_or(StoreError::Permission)?.join(format!(
            ".publish-{}",
            super::random_id().map_err(|_| StoreError::Unavailable)?
        ));
        storage::atomic_write(&temporary, bytes).map_err(failure)?;
        // Same-volume exclusive rename is atomic and never exposes an nlink=2 object.
        if !self.lock.protects(&self.root) {
            let _ = fs::remove_file(&temporary);
            return Err(StoreError::Permission);
        }
        let from =
            CString::new(temporary.as_os_str().as_bytes()).map_err(|_| StoreError::Permission)?;
        let to = CString::new(target.as_os_str().as_bytes()).map_err(|_| StoreError::Permission)?;
        // SAFETY: both NUL-terminated paths remain alive; RENAME_EXCL refuses any destination.
        let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        if result == 0 {
            return storage::sync_directory(target.parent().ok_or(StoreError::Permission)?)
                .map_err(failure);
        }
        let error = io::Error::last_os_error();
        let _ = fs::remove_file(&temporary);
        if error.kind() == io::ErrorKind::AlreadyExists {
            match self.read(key, ads::MAX_IMAGE_BYTES)? {
                Some(existing) if existing.bytes == bytes => {
                    storage::sync_directory(target.parent().ok_or(StoreError::Permission)?)
                        .map_err(failure)
                }
                _ => Err(StoreError::Conflict),
            }
        } else {
            Err(failure(error))
        }
    }
    fn commit_current(
        &mut self,
        expected_etag: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        if bytes.len() > ads::MAX_MANIFEST_BYTES {
            return Err(StoreError::Permission);
        }
        let current = self.current()?;
        if current.as_ref().map(|o| o.etag.as_str()) != expected_etag {
            return Err(StoreError::Conflict);
        }
        let path = self.path(&protocol::current_key(self.channel))?;
        // An fsync error may follow the rename commit point. The caller must reconcile.
        storage::atomic_write(&path, bytes).map_err(|_| StoreError::OutcomeUnknown)
    }
}
