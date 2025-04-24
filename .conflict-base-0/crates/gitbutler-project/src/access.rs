use anyhow::{bail, Context};
use parking_lot::RawRwLock;
use std::path::PathBuf;
use std::{collections::BTreeMap, sync::Arc};

use crate::{Project, ProjectId};

/// Access Control
impl Project {
    /// Try to obtain the exclusive inter-process lock on the entire project, preventing other GitButler
    /// instances to operate on it entirely.
    /// This lock should be obtained and held for as long as a user interface is observing the project.
    ///
    /// Note that the lock is automatically released on `Drop`, or when the process quits for any reason,
    /// so it can't go stale.
    pub fn try_exclusive_access(&self) -> anyhow::Result<LockFile> {
        // MIGRATION: bluntly remove old lock files, which are now more generally named to also fit
        //            the CLI.
        std::fs::remove_file(self.gb_dir().join("window.lock").as_os_str()).ok();

        let mut lock = LockFile::open(self.gb_dir().join("project.lock").as_os_str())?;
        let got_lock = lock
            .try_lock()
            .context("Failed to check if lock is taken")?;
        if !got_lock {
            bail!(
                "Project '{}' is already opened in another window",
                self.title
            );
        }
        Ok(lock)
    }

    /// Return a guard for exclusive (read+write) worktree access, blocking while waiting for someone else,
    /// in the same process only, to release it, or for all readers to disappear.
    /// Locking is fair.
    ///
    /// Note that this in-process locking works only under the assumption that no two instances of
    /// GitButler are able to read or write the same repository.
    pub fn exclusive_worktree_access(&self) -> WriteWorkspaceGuard {
        let mut map = WORKTREE_LOCKS.lock();
        WriteWorkspaceGuard {
            _inner: map.entry(self.id).or_default().write_arc(),
            perm: WorktreeWritePermission(()),
        }
    }

    /// Return a guard for shared (read) worktree access, and block while waiting for writers to disappear.
    /// There can be multiple readers, but only a single writer. Waiting writers will be handled with priority,
    /// thus block readers to prevent writer starvation.
    pub fn shared_worktree_access(&self) -> WorkspaceReadGuard {
        let mut map = WORKTREE_LOCKS.lock();
        WorkspaceReadGuard(map.entry(self.id).or_default().read_arc())
    }
}

pub struct WriteWorkspaceGuard {
    _inner: parking_lot::ArcRwLockWriteGuard<RawRwLock, ()>,
    perm: WorktreeWritePermission,
}

impl WriteWorkspaceGuard {
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

pub struct WorkspaceReadGuard(#[allow(dead_code)] parking_lot::ArcRwLockReadGuard<RawRwLock, ()>);

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

static WORKTREE_LOCKS: parking_lot::Mutex<BTreeMap<ProjectId, Arc<parking_lot::RwLock<()>>>> =
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
