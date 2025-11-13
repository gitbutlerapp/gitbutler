//! A crate to host a `Context` type, suitable to provide the context *applications* need to operate
//! on it.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use std::path::{Path, PathBuf};

use but_settings::AppSettings;
#[cfg(feature = "legacy")]
use gitbutler_command_context::CommandContext;

/// A context specific to a repository(project) specific information, *not* thread-safe, and cheap to clone.
/// That way it may own per-thread caches.
#[derive(Debug, Clone)]
pub struct Context {
    /// The application context, here for convenience and as feature toggles and flags are needed.
    pub settings: AppSettings,
    /// The legacy implementation, for all the old code.
    #[cfg(feature = "legacy")]
    pub legacy_project: gitbutler_project::Project,
    /// The repository of the project, which also provides access to the `git_dir`.
    pub repo: gix::Repository,
}

/// Legacy - none of this should be kept.
// TODO: make this an extension implemented elsewhere.
#[cfg(feature = "legacy")]
impl Context {
    /// Return a context for calling into `gitbutler-` functions.
    pub fn legacy_ctx(&self) -> anyhow::Result<CommandContext> {
        CommandContext::open(&self.legacy_project, self.settings.clone())
    }

    /// Return a wrapper for metadata that only supports read-only access when presented with the project wide permission
    /// to read data.
    /// This is helping to prevent races with mutable instances.
    // TODO: remove _read_only as we don't need it anymore with a DB based implementation as long as the instances
    //       starts a transaction to isolate reads.
    //       For a correct implementation, this would also have to hold on to `_read_only`.
    pub fn legacy_meta(
        &self,
        _read_only: &but_core::sync::WorktreeReadPermission,
    ) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.legacy_project.gb_dir().join("virtual_branches.toml"),
        )
    }
}

/// Locking utilities to protect against concurrency on the same repo.
impl Context {
    /// Return a guard for exclusive (read+write) worktree access, blocking while waiting for someone else,
    /// in the same process only, to release it, or for all readers to disappear.
    /// Locking is fair.
    ///
    /// Note that this in-process locking works only under the assumption that no two instances of
    /// GitButler are able to read or write the same repository.
    pub fn exclusive_worktree_access(&self) -> but_core::sync::WorkspaceWriteGuard {
        but_core::sync::exclusive_worktree_access(self.git_dir())
    }

    /// Return a guard for shared (read) worktree access, and block while waiting for writers to disappear.
    /// There can be multiple readers, but only a single writer. Waiting writers will be handled with priority,
    /// thus block readers to prevent writer starvation.
    pub fn shared_worktree_access(&self) -> but_core::sync::WorkspaceReadGuard {
        but_core::sync::shared_worktree_access(self.git_dir())
    }
}

/// Utilities for calling into plumbing functions.
impl Context {
    /// Return a wrapper for metadata that only supports read-only access when presented with the project wide permission
    /// to read data.
    /// This is helping to prevent races with mutable instances.
    // TODO: remove _read_only as we don't need it anymore with a DB based implementation as long as the instances
    //       starts a transaction to isolate reads.
    //       For a correct implementation, this would also have to hold on to `_read_only`.
    pub fn meta(
        &self,
        _read_only: &but_core::sync::WorktreeReadPermission,
    ) -> anyhow::Result<impl but_core::RefMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.data_dir().join("virtual_branches.toml"),
        )
    }
}

/// Repository helpers.
impl Context {
    /// Open an isolated repository, one that didn't read options beyond `.git/config` and
    /// knows no environment variables.
    ///
    /// Use it for fastest-possible access, when incomplete configuration is acceptable.
    pub fn open_isolated_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open_opts(
            self.git_dir(),
            gix::open::Options::isolated(),
        )?)
    }

    /// Open a standard Git repository at the project directory, just like a real user would.
    ///
    /// This repository is good for standard tasks, like checking refs and traversing the commit graph,
    /// and for reading objects as well.
    ///
    /// Diffing and merging is better done with [`Self::open_repo_for_merging()`].
    pub fn open_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open(self.git_dir())?)
    }

    /// Calls [`but_core::open_repo_for_merging()`]
    pub fn open_repo_for_merging(&self) -> anyhow::Result<gix::Repository> {
        but_core::open_repo_for_merging(self.git_dir())
    }

    fn git_dir(&self) -> &Path {
        self.repo.git_dir()
    }

    fn data_dir(&self) -> PathBuf {
        self.git_dir().join("gitbutler")
    }
}
