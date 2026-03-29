use but_core::sync::LockScope::AllOperations;
pub use but_core::sync::{
    LockFile, RepoExclusive, RepoExclusiveGuard, RepoShared, RepoSharedGuard,
};

use crate::Context;

/// Locking utilities to protect against concurrency on the same repo.
impl Context {
    /// Try to obtain the exclusive inter-process lock on the entire project, preventing other GitButler
    /// instances to operate on it entirely.
    /// This lock should be obtained and held for as long as a user interface is observing the project.
    ///
    /// Note that the lock is automatically released on `Drop`, or when the process quits for any reason,
    /// so it can't go stale.
    ///
    /// # IMPORTANT: KEEP THE LOCK ALIVE!
    pub fn try_exclusive_access(&mut self) -> anyhow::Result<LockFile> {
        but_core::sync::try_exclusive_inter_process_access(&self.gitdir, AllOperations)
    }
    /// Return a guard for exclusive (read+write) worktree access, blocking while waiting for
    /// someone else in the same process to release it, or for all readers to disappear.
    /// Locking is fair within this process.
    /// When `project_data_dir` is available we also attempt to obtain a best-effort
    /// inter-process file lock, but failures to create, open, or lock that file are logged and
    /// ignored, so the hard guarantee remains in-process exclusivity only.
    /// Note that this works on a shared reference as internal state isn't changed.
    ///
    /// # IMPORTANT: KEEP THE GUARD ALIVE!
    pub fn exclusive_worktree_access(&mut self) -> RepoExclusiveGuard {
        but_core::sync::exclusive_repo_access(&self.gitdir, Some(&self.project_data_dir))
    }

    /// Return a guard for shared (read) worktree access, and block while waiting for writers to disappear.
    /// There can be multiple readers, but only a single writer. Waiting writers will be handled with priority,
    /// thus block readers to prevent writer starvation.
    ///
    /// # IMPORTANT: KEEP THE GUARD ALIVE!
    pub fn shared_worktree_access(&self) -> RepoSharedGuard {
        but_core::sync::shared_repo_access(&self.gitdir)
    }
}
