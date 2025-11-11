use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, bail};
use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock};

/// Try to obtain the exclusive inter-process lock on the entire project that stores its application data in `project_data`,
/// preventing other GitButler instances to operate on it entirely.
/// This lock should be obtained and held for as long as a user interface is observing the project.
///
/// Note that the lock is automatically released on `Drop`, or when the process quits for any reason,
/// so it can't go stale.
pub fn try_exclusive_inter_process_access(
    project_data: impl AsRef<Path>,
) -> anyhow::Result<LockFile> {
    let project_data = project_data.as_ref();
    let mut lock = LockFile::open(project_data.join("project.lock").as_os_str())?;
    let got_lock = lock
        .try_lock()
        .context("Failed to check if lock is taken")?;
    if !got_lock {
        bail!(
            "Project at '{}' is already opened for writing",
            project_data.display()
        );
    }
    Ok(lock)
}

/// Return a guard for exclusive (read+write) worktree access for the project at `git_dir`,
/// blocking while waiting for someone else, in the same process only, to release it, or for all readers to disappear.
/// Locking is fair.
///
/// Note that this **in-process** locking works only under the assumption that no two instances of
/// GitButler are able to read or write the same repository.
pub fn exclusive_worktree_access(git_dir: impl Into<PathBuf>) -> WorkspaceWriteGuard {
    let mut map = WORKTREE_LOCKS.lock();
    let git_dir = git_dir.into();
    WorkspaceWriteGuard {
        inner: map.entry(git_dir).or_default().write_arc().into(),
        perm: WorktreeWritePermission(()),
    }
}

/// Return a guard for shared (read) worktree access for the project at `git_dir`,
/// and block while waiting for writers to disappear.
/// There can be multiple readers, but only a single writer. Waiting writers will be handled with priority,
/// thus block readers to prevent writer starvation.
pub fn shared_worktree_access(git_dir: impl Into<PathBuf>) -> WorkspaceReadGuard {
    let mut map = WORKTREE_LOCKS.lock();
    let git_dir = git_dir.into();
    WorkspaceReadGuard(Some(map.entry(git_dir).or_default().read_arc()))
}

/// A utility that drops an exclusive lock on drop.
pub struct WorkspaceWriteGuard {
    inner: Option<parking_lot::ArcRwLockWriteGuard<RawRwLock, ()>>,
    perm: WorktreeWritePermission,
}

impl Drop for WorkspaceWriteGuard {
    fn drop(&mut self) {
        let lock = self
            .inner
            .take()
            .expect("it's always set, and only taken once when dropping");
        ArcRwLockWriteGuard::unlock_fair(lock);
    }
}

impl Drop for WorkspaceReadGuard {
    fn drop(&mut self) {
        let lock = self
            .0
            .take()
            .expect("it's always set, and only taken once when dropping");
        ArcRwLockReadGuard::unlock_fair(lock)
    }
}

impl WorkspaceWriteGuard {
    /// Signal that a write-permission is available - useful as API-marker to assure these
    /// can only be called when the respective protection/permission is present.
    pub fn write_permission(&mut self) -> &mut WorktreeWritePermission {
        &mut self.perm
    }

    /// Signal that a read-permission is available - useful as API-marker to assure these
    /// can only be called when the respective protection/permission is present.
    pub fn read_permission(&self) -> &WorktreeReadPermission {
        self.perm.read_permission()
    }
}

/// A utility that drops a shared lock on drop.
pub struct WorkspaceReadGuard(Option<parking_lot::ArcRwLockReadGuard<RawRwLock, ()>>);

impl WorkspaceReadGuard {
    /// Signal that a read-permission is available - useful as API-marker to assure these
    /// can only be called when the respective protection/permission is present.
    pub fn read_permission(&self) -> &WorktreeReadPermission {
        static READ: WorktreeReadPermission = WorktreeReadPermission(());
        &READ
    }
}

/// A token to indicate read-only access was granted to the worktree, assuring there are no writers
/// *within this process*.
pub struct WorktreeReadPermission(());

/// A token to indicate exclusive access was granted to the worktree, assuring there are no readers or other writers
/// *within this process*.
pub struct WorktreeWritePermission(());

impl WorktreeWritePermission {
    /// Signal that a read-permission is available - useful as API-marker to assure these
    /// can only be called when the respective protection/permission is present.
    pub fn read_permission(&self) -> &WorktreeReadPermission {
        static READ: WorktreeReadPermission = WorktreeReadPermission(());
        &READ
    }
}

static WORKTREE_LOCKS: parking_lot::Mutex<BTreeMap<PathBuf, Arc<parking_lot::RwLock<()>>>> =
    parking_lot::Mutex::new(BTreeMap::new());

/// A file-based lock that can indicate exclusive access.
///
/// As opposed to its actual implementation, it will ignore failures due to lack of filesystem support.
pub struct LockFile {
    /// The actual lock implementation.
    inner: fslock::LockFile,
    /// The path which was originally locked, for error reporting.
    path: PathBuf,
}

/// Lifecycle and operations
impl LockFile {
    /// Open the file at `path`, possibly creating it, for the purpose of using it for file-based locking.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, fslock::Error> {
        let path = path.into();
        Ok(Self {
            inner: fslock::LockFile::open(path.as_path())?,
            path,
        })
    }

    /// Try to lock the resource, or pretend it was locked if the underlying filesystem didn't support it.
    pub fn try_lock(&mut self) -> Result<bool, fslock::Error> {
        self.inner.try_lock().or_else(|err| {
            if err.kind() == std::io::ErrorKind::Unsupported {
                tracing::warn!(
                "Filesystem hosting '{}' doesn't support file locking - pretending to own lock to avoid failure",
                self.path.display()
            );
                Ok(true)
            } else {
                Err(err)
            }
        })
    }

    /// Drop the lock on this file, or do nothing if we don't own the lock.
    pub fn unlock(&mut self) -> Result<(), fslock::Error> {
        if !self.inner.owns_lock() {
            return Ok(());
        }
        self.inner.unlock()
    }
}
