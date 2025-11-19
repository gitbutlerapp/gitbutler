//! A crate to host a `Context` type, suitable to provide the context *applications* need to operate on a Git repository.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use but_settings::AppSettings;
#[cfg(feature = "legacy")]
use gitbutler_command_context::CommandContext;
use std::path::{Path, PathBuf};

/// A UUID based project ID which is associated with metadata via `<app-dir>/projects.json`
///
/// The goal is to bring this metadata into `<project-data-dir>/`, and use `ProjectHandle` in future
/// which is self-describing and able to point to a path on disk while being URL safe.
#[cfg(feature = "legacy")]
pub type LegacyProjectId = gitbutler_project::ProjectId;

/// Project metadata and utilities to access it. Superseded by [`Context`].
#[cfg(feature = "legacy")]
pub type LegacyProject = gitbutler_project::Project;

/// A self-describing handle to the path of the project on disk, typically the `.git` directory of a Git repository.
///
/// With it, all project data and metadata can be accessed.
/// Further, this ID is URL-safe, but it is *not* for human consumption.
pub type ProjectHandle = String;

/// A context specific to a repository, along with commonly used information to make higher-level functions
/// more convenient to implement.
/// This type is *not* thread-safe, and cheap to clone. That way it may own per-thread caches.
/// It's fine for it to one day receive thread-safe shared state, as needed, similar to [`gix::Repository`].
#[derive(Debug, Clone)]
pub struct Context {
    /// The application context, here for convenience and as feature toggles and flags are needed.
    pub settings: AppSettings,
    /// The legacy implementation, for all the old code.
    #[cfg(feature = "legacy")]
    pub legacy_project: LegacyProject,
    /// The repository of the project, which also provides access to the `git_dir`.
    pub repo: gix::Repository,
}

/// Legacy Lifecycle
#[cfg(feature = "legacy")]
impl Context {
    /// Create a context from a legacy `project_id`,
    /// which requires reading `projects.json` to map it to metadata.
    pub fn new_from_legacy_project_id(project_id: LegacyProjectId) -> anyhow::Result<Self> {
        let legacy_project = gitbutler_project::get(project_id)?;
        let repo = gix::open(legacy_project.git_dir())?;
        Ok(Context {
            settings: AppSettings::load_from_default_path_creating()?,
            legacy_project,
            repo,
        })
    }

    /// Discover the Git repository in `directory` and return it as context.
    pub fn discover(directory: impl AsRef<Path>) -> anyhow::Result<Context> {
        let directory = directory.as_ref();
        let repo = gix::discover(directory)?;
        #[cfg(feature = "legacy")]
        {
            use anyhow::Context;
            let worktree_dir = repo
                .workdir()
                .context("Bare repositories aren't yet supported.")?;
            let project = LegacyProject::find_by_worktree_dir(worktree_dir)?;
            Ok(crate::Context {
                settings: AppSettings::load_from_default_path_creating()?,
                legacy_project: project,
                repo,
            })
        }

        #[cfg(not(feature = "legacy"))]
        {
            Ok(crate::Context {
                settings: AppSettings::load_from_default_path_creating()?,
                repo,
            })
        }
    }
}

/// Legacy - none of this should be kept.
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
            self.project_data_dir().join("virtual_branches.toml"),
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
        but_core::sync::exclusive_worktree_access(self.gitdir())
    }

    /// Return a guard for shared (read) worktree access, and block while waiting for writers to disappear.
    /// There can be multiple readers, but only a single writer. Waiting writers will be handled with priority,
    /// thus block readers to prevent writer starvation.
    pub fn shared_worktree_access(&self) -> but_core::sync::WorkspaceReadGuard {
        but_core::sync::shared_worktree_access(self.gitdir())
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
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }
}

/// Paths and locations.
impl Context {
    /// The location where project-specific data can be stored that is owned by the application.
    pub fn project_data_dir(&self) -> PathBuf {
        self.gitdir().join("gitbutler")
    }

    /// The path to the worktree directory or the `.git` directory if there is no worktree directory.
    pub fn workdir_or_gitdir(&self) -> &Path {
        self.repo.workdir().unwrap_or(self.repo.git_dir())
    }
}

/// Repository helpers, for when you need something more specific than [Self::repo].
impl Context {
    /// Open an isolated repository, one that didn't read options beyond `.git/config` and
    /// knows no environment variables.
    ///
    /// Use it for fastest-possible access, when incomplete configuration is acceptable.
    pub fn open_isolated_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open_opts(
            self.gitdir(),
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
        Ok(gix::open(self.gitdir())?)
    }

    /// Calls [`but_core::open_repo_for_merging()`]
    pub fn open_repo_for_merging(&self) -> anyhow::Result<gix::Repository> {
        but_core::open_repo_for_merging(self.gitdir())
    }

    /// The repository `.git` directory, and always the best paths to identify an actual repository
    /// (or repository associated with a submodule or worktree).
    fn gitdir(&self) -> &Path {
        self.repo.git_dir()
    }
}
