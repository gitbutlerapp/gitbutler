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
    // TODO(ctx): make it need &mut
    pub fn try_exclusive_access(&self) -> anyhow::Result<LockFile> {
        but_core::sync::try_exclusive_inter_process_access(&self.gitdir, AllOperations)
    }
    /// Return a guard for exclusive (read+write) worktree access, blocking while waiting for someone else,
    /// in the same process only, to release it, or for all readers to disappear.
    /// Locking is fair.
    /// Note that this works on a shared reference as internal state isn't changed.
    ///
    /// Note that this in-process locking works only under the assumption that no two instances of
    /// GitButler are able to read or write the same repository.
    ///
    /// # IMPORTANT: KEEP THE GUARD ALIVE!
    pub fn exclusive_worktree_access(&mut self) -> RepoExclusiveGuard {
        but_core::sync::exclusive_repo_access(&self.gitdir)
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
