use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context as _, bail};
use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock};
/// The scope of a lock. It can be either on the entire project or on specific operations.
#[derive(Debug, Clone, Copy, Default)]
pub enum LockScope {
    /// Represents a lock on the entire project, preventing other GitButler instances from operating on it.
    #[default]
    AllOperations,
    /// Represents a lock on background refresh operations only. This would prevent other GitButler instances
    /// from performing background refreshes, but would still allow exclusive access for user-driven operations.
    BackgroundRefreshOperations,
}

impl From<LockScope> for PathBuf {
    fn from(val: LockScope) -> Self {
        match val {
            LockScope::AllOperations => PathBuf::from("project.lock"),
            LockScope::BackgroundRefreshOperations => PathBuf::from("background-refresh.lock"),
        }
    }
}

/// Try to obtain an exclusive inter-process lock on a project that stores its application data in `project_data`.
///
/// The `scope` parameter determines what operations the lock protects:
/// - [`LockScope::AllOperations`]: Prevents other GitButler instances from performing any operations on the project.
///   This lock should be held for as long as a user interface is observing the project.
/// - [`LockScope::BackgroundRefreshOperations`]: Only prevents background refresh operations, allowing other
///   user-driven operations to proceed. This enables multiple GitButler instances to work on the same project
///   as long as only one is performing background refreshes at a time.
///
/// Returns an error if another process already holds the requested lock scope.
///
/// # Lock Lifecycle
///
/// The lock is automatically released when the returned [`LockFile`] is dropped, or when the process quits for any reason,
/// so it can't go stale.
pub fn try_exclusive_inter_process_access(
    project_data: impl AsRef<Path>,
    scope: LockScope,
) -> anyhow::Result<LockFile> {
    let project_data = project_data.as_ref();
    let mut lock = LockFile::open(project_data.join::<PathBuf>(scope.into()).as_os_str())?;
    let got_lock = lock
        .try_lock()
        .context("Failed to check if lock is taken")?;
    if !got_lock {
        let error_message = match scope {
            LockScope::AllOperations => {
                format!(
                    "Project at '{}' is already opened for writing by another GitButler instance",
                    project_data.display()
                )
            }
            LockScope::BackgroundRefreshOperations => {
                format!(
                    "Project at '{}' is already being refreshed in the background by another GitButler instance",
                    project_data.display()
                )
            }
        };
        bail!(error_message);
    }
    Ok(lock)
}

/// Return a guard for exclusive (read+write) *in-process* repository access for the project at
/// `git_dir`, blocking while waiting for someone else in this process to release it, or for all
/// readers to disappear. Locking is fair.
/// Also use `project_data_dir` if `Some` to create an *inter-process* exclusive lock.
/// Opening or locking that file is best-effort and failures are ignored, so the hard
/// guarantee provided by this function remains in-process exclusivity only.
/// If `project_data_dir` is `None`, no inter-process lock is obtained.
///
/// Use this only at the boundary where the lock is acquired. Keep the returned [`RepoExclusiveGuard`]
/// alive in the caller that owns the lock, and pass [`RepoExclusive`] further down instead via
/// [`RepoExclusiveGuard::write_permission()`].
pub fn exclusive_repo_access(
    git_dir: impl Into<PathBuf>,
    project_data_dir: Option<&Path>,
) -> RepoExclusiveGuard {
    let lock = {
        let git_dir = git_dir.into();
        let mut map = WORKTREE_LOCKS.lock();
        Arc::clone(map.entry(git_dir).or_default())
    };
    // The global `WORKTREE_LOCKS` mutex is released before blocking on the per-repo RwLock,
    // so contention on one repo cannot block access to unrelated repos.
    let in_process_lock = lock.write_arc();
    // After the in-process lock was obtained we know there is no in-process contention.
    // Now it's much safer to blockingly wait on the inter-porcess lock to protect against
    // multiple writers. Note that as a filesystem lock, no fairness is guaraneteed
    let inter_proces_lock_path = project_data_dir.map(|dir| dir.join("gitbutler.write-lock"));
    let mut inter_process_lock = inter_proces_lock_path.and_then(|path| LockFile::open(path).ok());
    if let Some(file_lock) = inter_process_lock.as_mut() {
        file_lock.lock().ok();
    }
    RepoExclusiveGuard {
        in_process_fair_lock: Some(in_process_lock),
        inter_process_unfair_lock: inter_process_lock,
        perm: RepoExclusive(()),
    }
}

/// Return a guard for shared (read) repository access for the project at `git_dir`,
/// and block while waiting for writers to disappear.
/// There can be multiple readers, but only a single writer. Waiting writers will be handled with priority,
/// thus block readers to prevent writer starvation.
///
/// Use this only at the boundary where the lock is acquired. Keep the returned [`RepoSharedGuard`]
/// alive in the caller that owns the lock, and pass [`RepoShared`] further down instead via
/// [`RepoSharedGuard::read_permission()`].
pub fn shared_repo_access(git_dir: impl Into<PathBuf>) -> RepoSharedGuard {
    let lock = {
        let mut map = WORKTREE_LOCKS.lock();
        let git_dir = git_dir.into();
        Arc::clone(map.entry(git_dir).or_default())
    };
    // The global `WORKTREE_LOCKS` mutex is released before blocking on the per-repo RwLock,
    // so contention on one repo cannot block access to unrelated repos.
    RepoSharedGuard(Some(lock.read_arc()))
}

/// Owns the *exclusive* in-process repository lock and the optional best-effort inter-process file
/// lock associated with it.
///
/// This type is for lock acquisition and lock lifetime management only.
/// Keep it in the top-level caller that actually acquires the lock, or face deadlocks.
///
/// Do not pass this type through application APIs unless the API itself is responsible for lock
/// ownership. Instead, derive a [`RepoExclusive`] with [`Self::write_permission()`] and pass that
/// permission token to lower-level functions while keeping the guard alive in the caller.
#[must_use]
pub struct RepoExclusiveGuard {
    in_process_fair_lock: Option<parking_lot::ArcRwLockWriteGuard<RawRwLock, ()>>,
    inter_process_unfair_lock: Option<LockFile>,
    perm: RepoExclusive,
}

impl Drop for RepoExclusiveGuard {
    fn drop(&mut self) {
        let lock = self
            .in_process_fair_lock
            .take()
            .expect("it's always set, and only taken once when dropping");
        ArcRwLockWriteGuard::unlock_fair(lock);
        if let Some(mut inter_process_lock) = self.inter_process_unfair_lock.take()
            && let Err(err) = inter_process_lock.unlock()
        {
            tracing::error!(?err, "Failed to release inter-process lock")
        }
    }
}

impl Drop for RepoSharedGuard {
    fn drop(&mut self) {
        let lock = self
            .0
            .take()
            .expect("it's always set, and only taken once when dropping");
        ArcRwLockReadGuard::unlock_fair(lock)
    }
}

impl RepoExclusiveGuard {
    /// Borrow the exclusive permission token tied to this guard.
    ///
    /// Pass the returned [`RepoExclusive`] to callees that require proof that the caller already
    /// holds the exclusive lock.
    pub fn write_permission(&mut self) -> &mut RepoExclusive {
        &mut self.perm
    }

    /// Borrow the shared read permission implied by exclusive access.
    ///
    /// This is useful for APIs that only need read access but still participate in the permission
    /// system to prevent concurrent writes.
    pub fn read_permission(&self) -> &RepoShared {
        self.perm.read_permission()
    }
}

/// Owns a *shared* in-process repository lock and releases it on drop.
///
/// This type is for lock acquisition and lock lifetime management only.
/// Keep it in the top-level caller that actually acquires the lock.
///
/// Do not pass this type through application APIs unless the API itself is responsible for lock
/// ownership. Instead, derive a [`RepoShared`] with [`Self::read_permission()`] and pass that
/// permission token to lower-level functions while keeping the guard alive in the caller.
#[must_use]
pub struct RepoSharedGuard(Option<ArcRwLockReadGuard<RawRwLock, ()>>);

impl RepoSharedGuard {
    /// Borrow the shared read permission token tied to this guard.
    ///
    /// Pass the returned [`RepoShared`] to callees that require proof that the caller already
    /// holds shared or exclusive read access.
    pub fn read_permission(&self) -> &RepoShared {
        static READ: RepoShared = RepoShared(());
        &READ
    }
}

/// A permission token proving read-only repository access was granted within this process.
///
/// Use this as a function parameter when a callee only needs proof that no writer is active.
/// It does not acquire or release locks by itself; that remains the job of [`RepoSharedGuard`] or
/// [`RepoExclusiveGuard`] in the caller.
pub struct RepoShared(());

/// A permission token proving exclusive repository access was granted within this process.
///
/// Use this as a function parameter when a callee needs proof that no other reader or writer is
/// active.
/// It does not acquire or release locks by itself; that remains the job of [`RepoExclusiveGuard`] in the caller.
pub struct RepoExclusive(());

impl RepoExclusive {
    /// Borrow the read permission implied by exclusive access.
    ///
    /// This allows code that only needs read access to accept [`RepoShared`] even when the caller
    /// holds exclusive access.
    pub fn read_permission(&self) -> &RepoShared {
        static READ: RepoShared = RepoShared(());
        &READ
    }
}

static WORKTREE_LOCKS: parking_lot::Mutex<BTreeMap<PathBuf, Arc<parking_lot::RwLock<()>>>> =
    parking_lot::Mutex::new(BTreeMap::new());

/// A file-based lock that can indicate exclusive access.
///
/// As opposed to its actual implementation, it will ignore failures due to lack of filesystem support.
/// It also prints traces using the path to the locked file to aid observability.
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
    /// Lock the resource, and block until the lock was obtained. Note that this will deadlock if the same process tries
    /// to obtain the same lock.
    ///
    /// Pretend it was locked if the underlying filesystem didn't support it.
    pub fn lock(&mut self) -> Result<(), fslock::Error> {
        self.inner
            .lock()
            .or_else(|err| log_error_if_unsupported(err, &self.path).map(|_| ()))
    }

    /// Try to lock the resource, or pretend it was locked if the underlying filesystem didn't support it.
    pub fn try_lock(&mut self) -> Result<bool, fslock::Error> {
        self.inner
            .try_lock()
            .or_else(|err| log_error_if_unsupported(err, &self.path))
    }

    /// Drop the lock on this file, or do nothing if we don't own the lock.
    pub fn unlock(&mut self) -> Result<(), fslock::Error> {
        if !self.inner.owns_lock() {
            return Ok(());
        }
        self.inner.unlock()
    }
}

fn log_error_if_unsupported(err: std::io::Error, self_path: &Path) -> Result<bool, std::io::Error> {
    if err.kind() == std::io::ErrorKind::Unsupported {
        tracing::warn!(
            "Filesystem hosting '{}' doesn't support file locking - pretending to own exclusive lock to avoid possibly needless failure.
            Consider using {gb_storage_path_key} to move project data to a different location",
            self_path.display(),
            gb_storage_path_key = but_project_handle::storage_path_config_key(),
        );
        Ok(true)
    } else {
        Err(err)
    }
}
