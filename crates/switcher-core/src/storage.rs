//! Shared lifecycle locking and durable, private metadata writes.
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
pub const METADATA_LIMIT: u64 = 64 * 1024;

/// Held for account writes, launch and installation. Stop requests never take this lock.
#[derive(Debug)]
pub struct LifecycleLock {
    _file: File,
    directory: PathBuf,
    directory_identity: (u64, u64),
    name: &'static str,
}

impl Drop for LifecycleLock {
    fn drop(&mut self) {
        // Explicit unlock also releases a descriptor briefly inherited by another thread's fork/exec.
        let _ = self._file.unlock();
    }
}

impl LifecycleLock {
    pub fn try_acquire(directory: &Path) -> io::Result<Self> {
        Self::try_named(directory, "lifecycle.lock", true)
    }

    fn try_named(directory: &Path, name: &'static str, create: bool) -> io::Result<Self> {
        validate_no_symlinks(directory)?;
        let directory = directory.canonicalize()?;
        let metadata = fs::metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o022 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe lifecycle directory",
            ));
        }
        let directory_identity = (metadata.dev(), metadata.ino());
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(create)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory.join(name))?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe lifecycle lock",
            ));
        }
        if name != "lifecycle.lock" {
            crate::accounts::access_control(&directory)?;
            crate::accounts::access_control(&directory.join(name))?;
        }
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => {
                io::Error::new(io::ErrorKind::WouldBlock, "lifecycle busy")
            }
            std::fs::TryLockError::Error(error) => error,
        })?;
        let lock = Self {
            _file: file,
            directory,
            directory_identity,
            name,
        };
        if !lock.protects(&lock.directory) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "lifecycle directory or lock was replaced",
            ));
        }
        Ok(lock)
    }

    pub fn protects(&self, directory: &Path) -> bool {
        if validate_no_symlinks(directory).is_err()
            || !directory
                .canonicalize()
                .is_ok_and(|path| path == self.directory)
        {
            return false;
        }
        let Ok(metadata) = fs::metadata(directory) else {
            return false;
        };
        let Ok(on_disk) = fs::symlink_metadata(directory.join(self.name)) else {
            return false;
        };
        let Ok(held) = self._file.metadata() else {
            return false;
        };
        (metadata.dev(), metadata.ino()) == self.directory_identity
            && (on_disk.dev(), on_disk.ino()) == (held.dev(), held.ino())
            && on_disk.is_file()
            && !on_disk.file_type().is_symlink()
            && on_disk.nlink() == 1
            && on_disk.uid() == unsafe { libc::geteuid() }
            && on_disk.mode() & 0o077 == 0
            && (self.name == "lifecycle.lock"
                || crate::accounts::access_control(&directory.join(self.name)).is_ok())
    }
}

/// Serializes Guardian health publication without taking the lifecycle lock.
pub(crate) struct GuardianSnapshotLock {
    _lock: LifecycleLock,
}
impl GuardianSnapshotLock {
    pub(crate) fn try_acquire(directory: &Path, create: bool) -> io::Result<Self> {
        LifecycleLock::try_named(directory, "guardian-snapshot.lock", create)
            .map(|lock| Self { _lock: lock })
    }
    #[cfg(test)]
    fn protects(&self, directory: &Path) -> bool {
        self._lock.protects(directory)
    }
}

/// Reject path traversal and symlinked ancestors before opening metadata or account roots.
pub fn validate_no_symlinks(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "absolute path required",
        ));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Normal(_) => current.push(component),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "non-normal path",
                ));
            }
        }
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "symlink or non-directory ancestor",
            ));
        }
    }
    Ok(())
}

/// Keep an existing legacy data root readable without changing its permissions.
/// All v2 descendants must still use create_private_directory (0700).
pub fn prepare_data_root(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return create_private_directory(path);
        }
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    validate_no_symlinks(path)?;
    validate_data_root_metadata(&fs::symlink_metadata(path)?, unsafe { libc::geteuid() })
}

fn validate_data_root_metadata(metadata: &fs::Metadata, owner: u32) -> io::Result<()> {
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.mode() & 0o022 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe existing data root",
        ));
    }
    Ok(())
}

pub fn create_private_directory(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent"))?;
    validate_no_symlinks(parent)?;
    match fs::create_dir(path) {
        Ok(()) => fs::set_permissions(path, fs::Permissions::from_mode(0o700))?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    validate_no_symlinks(path)?;
    let metadata = fs::metadata(path)?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private directory ownership or permissions",
        ));
    }
    sync_directory(parent)
}

pub fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

fn path_component(name: &std::ffi::OsStr) -> io::Result<std::ffi::CString> {
    std::ffi::CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}

// Bind each ancestor to an open directory. A later rename can move that inode;
// this protects against path redirection, not arbitrary same-user directory edits.
fn open_parent(path: &Path) -> io::Result<(File, std::ffi::CString)> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing filename"))?;
    if !parent.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "absolute path required",
        ));
    }
    let root = c"/";
    let fd = unsafe {
        libc::open(
            root.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut directory = unsafe { File::from_raw_fd(fd) };
    for component in parent.components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(name) => {
                #[cfg(test)]
                tests::before_component_open(name);
                let name = path_component(name)?;
                let fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    let error = io::Error::last_os_error();
                    if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "symlink or non-directory ancestor",
                        ));
                    }
                    return Err(error);
                }
                directory = unsafe { File::from_raw_fd(fd) };
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "non-normal path",
                ));
            }
        }
    }
    #[cfg(test)]
    tests::after_parent_checked();
    Ok((directory, path_component(name)?))
}

// This detects changes observed at the check; a same-user rename can still race it.
fn verify_parent_identity(path: &Path, parent: &File, committed: bool) -> io::Result<()> {
    let same = (|| {
        #[cfg(test)]
        tests::identity_check_fault()?;
        let held = parent.metadata()?;
        let (current, _) = open_parent(path)?;
        let current = current.metadata()?;
        Ok::<_, io::Error>(held.dev() == current.dev() && held.ino() == current.ino())
    })();
    match same {
        Ok(true) => Ok(()),
        Ok(false) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            if committed {
                "parent directory changed after rename; write may have committed"
            } else {
                "parent directory changed"
            },
        )),
        Err(error) if committed => {
            let kind = error.kind();
            Err(io::Error::new(kind, PostCommitError(error)))
        }
        Err(error) => Err(error),
    }
}

#[derive(Debug)]
struct PostCommitError(io::Error);

impl std::fmt::Display for PostCommitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "write may have committed: {}", self.0)
    }
}

impl std::error::Error for PostCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// The rename is the commit point. The file and its parent are both synchronized.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (parent, name) = open_parent(path)?;
    verify_parent_identity(path, &parent, false)?;
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    let status = unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if status == 0 {
        let metadata = unsafe { metadata.assume_init() };
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG
            || metadata.st_nlink != 1
            || metadata.st_uid != unsafe { libc::geteuid() }
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe metadata destination",
            ));
        }
    } else {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::NotFound {
            return Err(error);
        }
    }
    let serial = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    let temporary =
        std::ffi::CString::new(format!(".write-{}-{serial}", std::process::id())).unwrap();
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            temporary.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut committed = false;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        #[cfg(test)]
        tests::before_rename();
        verify_parent_identity(path, &parent, false)?;
        if unsafe {
            libc::renameat(
                parent.as_raw_fd(),
                temporary.as_ptr(),
                parent.as_raw_fd(),
                name.as_ptr(),
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        committed = true;
        #[cfg(test)]
        tests::after_commit();
        #[cfg(test)]
        tests::directory_sync_fault()
            .map_err(|error| io::Error::new(error.kind(), PostCommitError(error)))?;
        parent
            .sync_all()
            .map_err(|error| io::Error::new(error.kind(), PostCommitError(error)))?;
        verify_parent_identity(path, &parent, true)
    })();
    if result.is_err() && !committed {
        unsafe {
            libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0);
        }
    }
    result
}

pub fn atomic_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.len() as u64 > METADATA_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "metadata too large",
        ));
    }
    atomic_write(path, &bytes)
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let bytes = read_private_bytes(path, METADATA_LIMIT as usize)?;
    crate::signed::strict_json(&bytes, METADATA_LIMIT as usize).map_err(io::Error::other)
}

/// Bounded private-file reader for larger signed update state. Callers choose their protocol limit.
pub fn read_private_bytes(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let read_limit = u64::try_from(limit)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid private file limit"))?;
    let (parent, name) = open_parent(path)?;
    verify_parent_identity(path, &parent, false)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe private file source",
        ));
    }
    if metadata.len() >= read_limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "private file too large",
        ));
    }
    let mut bytes = Vec::new();
    file.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "private file too large",
        ));
    }
    #[cfg(test)]
    tests::after_read();
    verify_parent_identity(path, &parent, false)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    type ComponentHook = Box<dyn FnMut(&std::ffi::OsStr)>;

    thread_local! {
        static IDENTITY_FAULT: std::cell::RefCell<Option<io::Error>> =
            const { std::cell::RefCell::new(None) };
        static SYNC_FAULT: std::cell::RefCell<Option<io::Error>> =
            const { std::cell::RefCell::new(None) };
        static RENAME_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
            std::cell::RefCell::new(None);
        static PARENT_CHECK_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
            std::cell::RefCell::new(None);
        static COMPONENT_HOOK: std::cell::RefCell<Option<ComponentHook>> =
            std::cell::RefCell::new(None);
        static COMMIT_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
            std::cell::RefCell::new(None);
        static READ_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
            std::cell::RefCell::new(None);
    }

    pub(super) fn before_rename() {
        if let Some(hook) = RENAME_HOOK.with(|slot| slot.borrow_mut().take()) {
            hook();
        }
    }
    pub(super) fn directory_sync_fault() -> io::Result<()> {
        match SYNC_FAULT.with(|slot| slot.borrow_mut().take()) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(super) fn identity_check_fault() -> io::Result<()> {
        match IDENTITY_FAULT.with(|slot| slot.borrow_mut().take()) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    #[test]
    fn identity_check_preserves_underlying_system_error() {
        let root = RaceRoot::new();
        let path = root.0.join("parent/state");
        let (parent, _) = open_parent(&path).unwrap();
        IDENTITY_FAULT
            .with(|slot| *slot.borrow_mut() = Some(io::Error::from_raw_os_error(libc::EIO)));
        let error = verify_parent_identity(&path, &parent, false).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EIO));
    }

    #[test]
    fn identity_check_after_commit_preserves_system_error_source() {
        let root = RaceRoot::new();
        let path = root.0.join("parent/state");
        let (parent, _) = open_parent(&path).unwrap();
        IDENTITY_FAULT
            .with(|slot| *slot.borrow_mut() = Some(io::Error::from_raw_os_error(libc::EIO)));
        let error = verify_parent_identity(&path, &parent, true).unwrap_err();
        assert!(error.to_string().contains("may have committed"));
        assert_eq!(error.kind(), io::Error::from_raw_os_error(libc::EIO).kind());
        assert_eq!(
            error
                .get_ref()
                .and_then(|source| source.downcast_ref::<PostCommitError>())
                .and_then(|source| source.0.raw_os_error()),
            Some(libc::EIO)
        );
    }

    #[test]
    fn parent_moved_before_rename_keeps_old_data_and_removes_temporary() {
        let root = RaceRoot::new();
        let destination = root.0.join("parent/state");
        atomic_write(&destination, b"original").unwrap();
        let base = root.0.clone();
        RENAME_HOOK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                fs::rename(base.join("parent"), base.join("moved")).unwrap();
                std::os::unix::fs::symlink(base.join("decoy"), base.join("parent")).unwrap();
            }))
        });
        assert!(atomic_write(&destination, b"updated").is_err());
        assert_eq!(fs::read(root.0.join("moved/state")).unwrap(), b"original");
        assert_eq!(fs::read_dir(root.0.join("moved")).unwrap().count(), 1);
        assert!(!root.0.join("decoy/state").exists());
    }

    #[test]
    fn failed_directory_sync_reports_possible_commit_and_retains_data() {
        let root = RaceRoot::new();
        let destination = root.0.join("parent/state");
        atomic_write(&destination, b"original").unwrap();
        SYNC_FAULT.with(|slot| *slot.borrow_mut() = Some(io::Error::from_raw_os_error(libc::EIO)));
        let error = atomic_write(&destination, b"updated").unwrap_err();
        assert!(error.to_string().contains("may have committed"));
        assert_eq!(
            error
                .get_ref()
                .and_then(|source| source.downcast_ref::<PostCommitError>())
                .and_then(|source| source.0.raw_os_error()),
            Some(libc::EIO)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"updated");
    }

    pub(super) fn after_read() {
        let hook = READ_HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook();
        }
    }

    pub(super) fn after_commit() {
        let hook = COMMIT_HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook();
        }
    }

    pub(super) fn before_component_open(name: &std::ffi::OsStr) {
        COMPONENT_HOOK.with(|slot| {
            if let Some(hook) = slot.borrow_mut().as_mut() {
                hook(name);
            }
        });
    }

    pub(super) fn after_parent_checked() {
        let hook = PARENT_CHECK_HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook();
        }
    }

    struct RaceRoot(PathBuf);

    impl RaceRoot {
        fn new() -> Self {
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "switcher-parent-race-{}-{}",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            create_private_directory(&root).unwrap();
            create_private_directory(&root.join("parent")).unwrap();
            create_private_directory(&root.join("decoy")).unwrap();
            Self(root)
        }

        fn swap_after_check(&self) {
            let root = self.0.clone();
            PARENT_CHECK_HOOK.with(|slot| {
                assert!(slot.borrow().is_none());
                *slot.borrow_mut() = Some(Box::new(move || {
                    fs::rename(root.join("parent"), root.join("moved")).unwrap();
                    std::os::unix::fs::symlink(root.join("decoy"), root.join("parent")).unwrap();
                }));
            });
        }
    }

    impl Drop for RaceRoot {
        fn drop(&mut self) {
            PARENT_CHECK_HOOK.with(|slot| {
                slot.borrow_mut().take();
            });
            COMPONENT_HOOK.with(|slot| {
                slot.borrow_mut().take();
            });
            COMMIT_HOOK.with(|slot| {
                slot.borrow_mut().take();
            });
            READ_HOOK.with(|slot| {
                slot.borrow_mut().take();
            });
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn atomic_write_stays_in_opened_parent_after_path_is_replaced() {
        let root = RaceRoot::new();
        let destination = root.0.join("parent/state");
        let decoy = root.0.join("decoy/state");
        atomic_write(&destination, b"original").unwrap();
        atomic_write(&decoy, b"decoy").unwrap();
        root.swap_after_check();
        let result = atomic_write(&destination, b"updated");
        assert_eq!(fs::read(&decoy).unwrap(), b"decoy");
        assert!(result.is_err());
        assert_eq!(fs::read(root.0.join("moved/state")).unwrap(), b"original");
    }

    #[test]
    fn private_read_stays_in_opened_parent_after_path_is_replaced() {
        let root = RaceRoot::new();
        let source = root.0.join("parent/state");
        atomic_write(&source, b"original").unwrap();
        atomic_write(&root.0.join("decoy/state"), b"decoy").unwrap();
        root.swap_after_check();
        assert!(read_private_bytes(&source, 32).is_err());
    }

    #[test]
    fn moved_parent_must_not_report_write_success() {
        let root = RaceRoot::new();
        let destination = root.0.join("parent/state");
        atomic_write(&destination, b"original").unwrap();
        root.swap_after_check();
        assert!(atomic_write(&destination, b"updated").is_err());
    }

    #[test]
    fn moved_parent_must_not_report_read_success() {
        let root = RaceRoot::new();
        let source = root.0.join("parent/state");
        atomic_write(&source, b"original").unwrap();
        root.swap_after_check();
        assert!(read_private_bytes(&source, 32).is_err());
    }

    #[test]
    fn parent_moved_after_commit_reports_possible_commit() {
        let root = RaceRoot::new();
        let destination = root.0.join("parent/state");
        atomic_write(&destination, b"original").unwrap();
        let base = root.0.clone();
        COMMIT_HOOK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                fs::rename(base.join("parent"), base.join("moved")).unwrap();
                std::os::unix::fs::symlink(base.join("decoy"), base.join("parent")).unwrap();
            }))
        });
        let error = atomic_write(&destination, b"updated").unwrap_err();
        assert!(error.to_string().contains("may have committed"));
        assert_eq!(fs::read(root.0.join("moved/state")).unwrap(), b"updated");
        assert!(!root.0.join("decoy/state").exists());
    }

    #[test]
    fn parent_moved_after_read_is_rejected() {
        let root = RaceRoot::new();
        let source = root.0.join("parent/state");
        atomic_write(&source, b"original").unwrap();
        let base = root.0.clone();
        READ_HOOK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                fs::rename(base.join("parent"), base.join("moved")).unwrap();
                std::os::unix::fs::symlink(base.join("decoy"), base.join("parent")).unwrap();
            }))
        });
        assert!(read_private_bytes(&source, 32).is_err());
    }

    #[test]
    fn opening_an_ancestor_rejects_a_concurrent_symlink_replacement() {
        let root = RaceRoot::new();
        let source = root.0.join("parent/state");
        atomic_write(&source, b"original").unwrap();
        atomic_write(&root.0.join("decoy/state"), b"decoy").unwrap();
        let base = root.0.clone();
        COMPONENT_HOOK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move |name| {
                if name == "parent" {
                    fs::rename(base.join("parent"), base.join("moved")).unwrap();
                    std::os::unix::fs::symlink(base.join("decoy"), base.join("parent")).unwrap();
                }
            }))
        });
        assert_eq!(
            read_private_bytes(&source, 32).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn anchored_io_preserves_missing_link_and_private_file_checks() {
        let root = RaceRoot::new();
        let source = root.0.join("parent/state");
        assert_eq!(
            read_private_bytes(&source, 8).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        let absent = root.0.join("missing/state");
        assert_eq!(
            atomic_write(&absent, b"value").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        atomic_write(&source, b"value").unwrap();
        let hardlink = root.0.join("parent/hardlink");
        fs::hard_link(&source, &hardlink).unwrap();
        assert_eq!(
            atomic_write(&source, b"new").unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            read_private_bytes(&source, 8).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read(&hardlink).unwrap(), b"value");
        let link = root.0.join("linked");
        std::os::unix::fs::symlink(root.0.join("parent"), &link).unwrap();
        assert_eq!(
            atomic_write(&link.join("state"), b"new")
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            read_private_bytes(&link.join("state"), 8)
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }
    #[test]
    fn legacy_data_root_keeps_modes_and_rejects_unsafe_metadata() {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-legacy-root-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        prepare_data_root(&root).unwrap();
        assert_eq!(fs::metadata(&root).unwrap().mode() & 0o777, 0o700);
        for mode in [0o755, 0o750, 0o700] {
            fs::set_permissions(&root, fs::Permissions::from_mode(mode)).unwrap();
            prepare_data_root(&root).unwrap();
            assert_eq!(fs::metadata(&root).unwrap().mode() & 0o777, mode);
        }
        for mode in [0o775, 0o757, 0o777] {
            fs::set_permissions(&root, fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(
                prepare_data_root(&root).unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
            assert_eq!(fs::metadata(&root).unwrap().mode() & 0o777, mode);
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        let metadata = fs::symlink_metadata(&root).unwrap();
        // The same real metadata must fail when its UID differs from the required owner.
        assert_eq!(
            validate_data_root_metadata(&metadata, metadata.uid().wrapping_add(1))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        let link = root.with_extension("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        assert_eq!(
            prepare_data_root(&link).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::remove_file(link).unwrap();
        let file = root.join("not-directory");
        atomic_write(&file, b"unchanged").unwrap();
        assert_eq!(
            prepare_data_root(&file).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read(file).unwrap(), b"unchanged");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn lifecycle_lock_excludes_another_owner_and_os_releases_it() {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-lock-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_directory(&path).unwrap();
        let lock = LifecycleLock::try_acquire(&path).unwrap();
        assert_eq!(
            LifecycleLock::try_acquire(&path).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(lock.protects(&path));
        drop(lock);
        LifecycleLock::try_acquire(&path).unwrap();
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn guardian_snapshot_lock_read_only_and_unsafe_paths() {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-snapshot-lock-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_directory(&path).unwrap();
        let control = path.join("guardian-snapshot.lock");
        assert!(GuardianSnapshotLock::try_acquire(&path, false).is_err());
        assert!(!control.exists());
        let held = GuardianSnapshotLock::try_acquire(&path, true).unwrap();
        assert!(held.protects(&path));
        assert!(GuardianSnapshotLock::try_acquire(&path, false).is_err());
        let lifecycle = LifecycleLock::try_acquire(&path).unwrap();
        assert!(lifecycle.protects(&path));
        fs::rename(&control, path.join("old-lock")).unwrap();
        drop(GuardianSnapshotLock::try_acquire(&path, true).unwrap());
        assert!(!held.protects(&path));
        drop(held);
        fs::set_permissions(&control, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(GuardianSnapshotLock::try_acquire(&path, false).is_err());
        fs::remove_file(&control).unwrap();
        std::os::unix::fs::symlink(path.join("old-lock"), &control).unwrap();
        assert!(GuardianSnapshotLock::try_acquire(&path, false).is_err());
        fs::remove_file(&control).unwrap();
        fs::hard_link(path.join("old-lock"), &control).unwrap();
        assert!(GuardianSnapshotLock::try_acquire(&path, false).is_err());
        drop(lifecycle);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn atomic_metadata_replaces_privately_and_refuses_symlink_destination() {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-write-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_directory(&path).unwrap();
        let destination = path.join("state.json");
        atomic_json(&destination, &vec![1, 2]).unwrap();
        atomic_json(&destination, &vec![3]).unwrap();
        assert_eq!(read_json::<Vec<u8>>(&destination).unwrap(), vec![3]);
        assert_eq!(fs::metadata(&destination).unwrap().mode() & 0o777, 0o600);
        let link = path.join("link.json");
        std::os::unix::fs::symlink(&destination, &link).unwrap();
        assert_eq!(
            atomic_json(&link, &vec![4]).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(read_json::<Vec<u8>>(&destination).unwrap(), vec![3]);
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn lifecycle_lock_child_fixture() {
        let Some(directory) = std::env::var_os("SWITCHER_SYNTHETIC_LOCK_CHILD") else {
            return;
        };
        let directory = PathBuf::from(directory);
        let _lock = LifecycleLock::try_acquire(&directory).unwrap();
        atomic_write(&directory.join("ready"), b"ready").unwrap();
        loop {
            std::thread::park_timeout(std::time::Duration::from_secs(1));
        }
    }

    #[test]
    fn lifecycle_lock_is_released_when_the_holding_process_is_killed() {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-lock-death-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_directory(&path).unwrap();
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = ChildGuard(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "storage::tests::lifecycle_lock_child_fixture",
                    "--nocapture",
                ])
                .env("SWITCHER_SYNTHETIC_LOCK_CHILD", &path)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !path.join("ready").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            path.join("ready").exists(),
            "synthetic child did not acquire lock"
        );
        assert_eq!(
            LifecycleLock::try_acquire(&path).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let lock = LifecycleLock::try_acquire(&path).unwrap();
        drop(lock);
        fs::remove_dir_all(path).unwrap();
    }

    fn contention_wait(path: &Path) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !path.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "contention deadline: {}",
                path.file_name().unwrap().to_string_lossy()
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    fn contention_rejected(v2: &Path, marker: &str) {
        assert_eq!(
            LifecycleLock::try_acquire(v2).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        atomic_write(&v2.join(marker), b"rejected").unwrap();
    }
    #[test]
    fn lifecycle_contention_child_fixture() {
        let Some(base) = std::env::var_os("SWITCHER_CONTENTION_HOME") else {
            return;
        };
        let base = PathBuf::from(base);
        assert_eq!(
            std::env::var_os("HOME"),
            Some(base.clone().into_os_string())
        );
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(base.parent(), Some(temporary.as_path()));
        assert!(
            base.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("switcher-lock-contention-")
        );
        validate_no_symlinks(&base).unwrap();
        let metadata = fs::metadata(&base).unwrap();
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.mode() & 0o077, 0);
        let v2 = base.join(".claude-switcher/v2");
        let role = std::env::var("SWITCHER_CONTENTION_ROLE").unwrap();
        match role.as_str() {
            "gui" => {
                let _lock = LifecycleLock::try_acquire(&v2).unwrap();
                atomic_write(&v2.join("shared-state"), b"gui").unwrap();
                atomic_write(&v2.join("gui-held"), b"held").unwrap();
                contention_wait(&v2.join("release-gui"));
                panic!("GUI fixture should have been killed");
            }
            "cli" => {
                contention_rejected(&v2, "cli-denied-by-gui");
                contention_wait(&v2.join("retry-cli"));
                let _lock = LifecycleLock::try_acquire(&v2).unwrap();
                atomic_write(&v2.join("shared-state"), b"cli").unwrap();
                atomic_write(&v2.join("cli-held"), b"held").unwrap();
                contention_wait(&v2.join("release-cli"));
            }
            "update" => {
                contention_rejected(&v2, "update-denied-by-gui");
                contention_wait(&v2.join("retry-update-denied"));
                contention_rejected(&v2, "update-denied-by-cli");
                contention_wait(&v2.join("retry-update-acquire"));
                let _lock = LifecycleLock::try_acquire(&v2).unwrap();
                atomic_write(&v2.join("shared-state"), b"update").unwrap();
                atomic_write(&v2.join("update-acquired"), b"held").unwrap();
            }
            _ => panic!("unknown fixed contention role"),
        }
    }

    #[test]
    fn three_process_writers_contend_and_os_releases_killed_holder() {
        use std::os::unix::process::ExitStatusExt;
        struct TemporaryHome(PathBuf);
        impl Drop for TemporaryHome {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        fn spawn(home: &Path, role: &str) -> ChildGuard {
            ChildGuard(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "storage::tests::lifecycle_contention_child_fixture",
                        "--nocapture",
                    ])
                    .env("HOME", home)
                    .env("SWITCHER_CONTENTION_HOME", home)
                    .env("SWITCHER_CONTENTION_ROLE", role)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            )
        }
        fn wait_success(child: &mut ChildGuard) {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    assert!(status.success());
                    return;
                }
                assert!(std::time::Instant::now() < deadline, "child exit deadline");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        let home = TemporaryHome(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-lock-contention-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        )));
        create_private_directory(&home.0).unwrap();
        create_private_directory(&home.0.join(".claude-switcher")).unwrap();
        let v2 = home.0.join(".claude-switcher/v2");
        create_private_directory(&v2).unwrap();
        let mut gui = spawn(&home.0, "gui");
        contention_wait(&v2.join("gui-held"));
        let mut cli = spawn(&home.0, "cli");
        let mut update = spawn(&home.0, "update");
        assert_ne!(gui.0.id(), cli.0.id());
        assert_ne!(gui.0.id(), update.0.id());
        assert_ne!(cli.0.id(), update.0.id());
        contention_wait(&v2.join("cli-denied-by-gui"));
        contention_wait(&v2.join("update-denied-by-gui"));
        assert_eq!(fs::read(v2.join("shared-state")).unwrap(), b"gui");
        assert_eq!(
            LifecycleLock::try_acquire(&v2).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        println!("phase=gui-held cli=WouldBlock update=WouldBlock shared=gui");
        gui.0.kill().unwrap();
        assert_eq!(gui.0.wait().unwrap().signal(), Some(libc::SIGKILL));
        atomic_write(&v2.join("retry-cli"), b"retry").unwrap();
        contention_wait(&v2.join("cli-held"));
        assert_eq!(fs::read(v2.join("shared-state")).unwrap(), b"cli");
        atomic_write(&v2.join("retry-update-denied"), b"retry").unwrap();
        contention_wait(&v2.join("update-denied-by-cli"));
        assert_eq!(fs::read(v2.join("shared-state")).unwrap(), b"cli");
        assert_eq!(
            LifecycleLock::try_acquire(&v2).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        println!("phase=gui-SIGKILL cli=acquired update=WouldBlock shared=cli");
        atomic_write(&v2.join("release-cli"), b"release").unwrap();
        wait_success(&mut cli);
        atomic_write(&v2.join("retry-update-acquire"), b"retry").unwrap();
        contention_wait(&v2.join("update-acquired"));
        wait_success(&mut update);
        assert_eq!(fs::read(v2.join("shared-state")).unwrap(), b"update");
        let lock = LifecycleLock::try_acquire(&v2).unwrap();
        assert!(lock.protects(&v2));
        drop(lock);
        println!("phase=cli-released update=acquired final-lock=available shared=update");
    }

    #[test]
    fn lock_identity_replacement_and_duplicate_json_are_rejected() {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-lock-identity-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_directory(&path).unwrap();
        let lock = LifecycleLock::try_acquire(&path).unwrap();
        fs::rename(path.join("lifecycle.lock"), path.join("old-lock")).unwrap();
        atomic_write(&path.join("lifecycle.lock"), b"").unwrap();
        assert!(!lock.protects(&path));
        atomic_write(&path.join("state.json"), br#"{"state":1,"state":2}"#).unwrap();
        assert!(read_json::<serde_json::Value>(&path.join("state.json")).is_err());
        drop(lock);
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn bounded_private_reads_reject_oversize_public_modes_and_fifo() {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-read-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_directory(&path).unwrap();
        let data = path.join("data");
        atomic_write(&data, b"12345").unwrap();
        assert_eq!(read_private_bytes(&data, 5).unwrap(), b"12345");
        assert_eq!(
            read_private_bytes(&data, 4).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::set_permissions(&data, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            read_private_bytes(&data, 5).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        let fifo = path.join("fifo");
        let c_path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        assert_eq!(
            read_private_bytes(&fifo, 5).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::remove_dir_all(path).unwrap();
    }
}
