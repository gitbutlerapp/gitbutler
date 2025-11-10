use crate::Project;

/// Access Control
impl Project {
    /// Try to obtain the exclusive inter-process lock on the entire project, preventing other GitButler
    /// instances to operate on it entirely.
    /// This lock should be obtained and held for as long as a user interface is observing the project.
    ///
    /// Note that the lock is automatically released on `Drop`, or when the process quits for any reason,
    /// so it can't go stale.
    pub fn try_exclusive_access(&self) -> anyhow::Result<but_core::sync::LockFile> {
        but_core::sync::try_exclusive_inter_process_access(self.git_dir())
    }

    /// Return a guard for exclusive (read+write) worktree access, blocking while waiting for someone else,
    /// in the same process only, to release it, or for all readers to disappear.
    /// Locking is fair.
    ///
    /// Note that this in-process locking works only under the assumption that no two instances of
    /// GitButler are able to read or write the same repository.
    pub fn exclusive_worktree_access(&self) -> WorkspaceWriteGuard {
        but_core::sync::exclusive_worktree_access(self.git_dir())
    }

    /// Return a guard for shared (read) worktree access, and block while waiting for writers to disappear.
    /// There can be multiple readers, but only a single writer. Waiting writers will be handled with priority,
    /// thus block readers to prevent writer starvation.
    pub fn shared_worktree_access(&self) -> WorkspaceReadGuard {
        but_core::sync::shared_worktree_access(self.git_dir())
    }
}

pub use but_core::sync::{
    LockFile, WorkspaceReadGuard, WorkspaceWriteGuard, WorktreeReadPermission,
    WorktreeWritePermission,
};
