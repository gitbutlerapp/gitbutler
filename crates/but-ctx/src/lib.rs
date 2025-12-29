//! A crate to host a `Context` type, suitable to provide the context *applications* need to operate on a Git repository.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use but_core::RepositoryExt;
use but_core::sync::{WorktreeReadPermission, WorktreeWritePermission};
use but_settings::AppSettings;
use std::path::{Path, PathBuf};

/// Legacy types that shouldn't be used.
#[cfg(feature = "legacy")]
pub mod legacy;
#[cfg(feature = "legacy")]
pub use legacy::types::{LegacyProject, LegacyProjectId};

/// Utilities to control access to the project directory of the context.
pub mod access;

/// A self-describing handle to the path of the project on disk, typically the `.git` directory of a Git repository.
///
/// With it, all project data and metadata can be accessed.
/// Further, this ID is URL-safe, but it is *not* for human consumption.
// TODO: needs actual implementation to make it usable in the `Context` API.
//       Needs implementation to turn it into a `PathBuf`, and to create it from a `Path`.
pub struct ProjectHandle(#[expect(dead_code)] String);

/// A context specific to a repository, along with commonly used information to make higher-level functions
/// more convenient to implement.
/// This type is *not* thread-safe, and cheap to clone. That way it may own per-thread caches.
///
/// It's fine for it to one day receive thread-safe shared state, as needed, similar to [`gix::Repository`].
///
/// ### Why Interior Mutability?
///
/// This is for ergonomics, to avoid having to set the context as `mut` for all uses effectively as it
/// would need to mutate itself to keep a cache.
/// As all items that it caches are more akin to 'connections' that have no inherent notion of mutability,
/// we provide mutable versions of these as well for maximum usability.
///
/// The idea is to only call into plumbing functions which *do care* about mutability, using standard ownership semantics.
///
/// ### Why everything pub?
///
/// Because we trust that you keep the invariant alive that everything in the Context is tied to `gitdir`, for that
/// extra-bit of convenience.
pub struct Context {
    /// The application context, here for convenience and as feature toggles and flags are needed.
    pub settings: AppSettings,
    /// The repository `.git` directory, and always the best paths to identify an actual repository
    /// (or repository associated with a submodule or worktree).
    pub gitdir: PathBuf,
    /// The most recently opened repository of the project, which also provides access to the `git_dir`.
    pub repo: OnDemand<gix::Repository>,
    /// The most recently opened `git2` repository of the project.
    pub git2_repo: OnDemand<git2::Repository>,
    /// An open handle to the database. It's initialized lazily upon first access.
    /// It is also what makes this type non-Clone, which is fair.
    pub db: OnDemand<but_db::DbHandle>,
    /// The legacy implementation, for all the old code.
    #[cfg(feature = "legacy")]
    pub legacy_project: LegacyProject,
}

/// A structure that can be passed across thread boundaries.
// TODO: make fields non-pub once `CommandContext` is gone.
#[derive(Clone)]
pub struct ThreadSafeContext {
    /// The application context, here for convenience and as feature toggles and flags are needed.
    pub settings: AppSettings,
    /// The directory at which the repository itself is located.
    pub gitdir: PathBuf,
    /// The most recently opened repository of the project, which also provides access to the `git_dir`.
    pub repo: Option<gix::ThreadSafeRepository>,
    /// The legacy implementation, for all the old code.
    #[cfg(feature = "legacy")]
    pub legacy_project: LegacyProject,
}

impl From<ThreadSafeContext> for Context {
    fn from(value: ThreadSafeContext) -> Self {
        let ThreadSafeContext {
            settings,
            gitdir,
            repo,
            #[cfg(feature = "legacy")]
            legacy_project,
        } = value;
        let mut ondemand = new_ondemand_repo(gitdir.clone());
        if let Some(repo) = repo {
            ondemand.assign(repo.to_thread_local());
        }
        Context {
            settings,
            repo: ondemand,
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(gitdir.clone()),
            gitdir,
            #[cfg(feature = "legacy")]
            legacy_project,
        }
    }
}

impl TryFrom<gix::Repository> for Context {
    type Error = anyhow::Error;

    fn try_from(repo: gix::Repository) -> Result<Self, Self::Error> {
        Context::from_repo(repo)
    }
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("git_dir", &self.gitdir)
            .finish()
    }
}

/// Lifecycle
impl Context {
    /// Create a new instance from just the `gitdir` of the repository we should provide context for.
    pub fn new(gitdir: impl Into<PathBuf>) -> anyhow::Result<Context> {
        let gitdir = gitdir.into();
        let settings = AppSettings::load_from_default_path_creating_without_customization()?;
        #[cfg(not(feature = "legacy"))]
        {
            Ok(Context {
                gitdir: gitdir.clone(),
                settings,
                repo: new_ondemand_repo(gitdir.clone()),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(gitdir),
            })
        }
        #[cfg(feature = "legacy")]
        {
            use anyhow::Context as _;
            let repo = gix::open(&gitdir)?;
            let worktree_dir = repo
                .workdir()
                .context("Bare repositories aren't yet supported.")?;
            let legacy_project = LegacyProject::find_by_worktree_dir(worktree_dir)
                .unwrap_or_else(|_| default_legacy_project_at_repo(&repo));
            Ok(Context {
                settings,
                gitdir: gitdir.clone(),
                legacy_project,
                repo: new_ondemand_repo(gitdir.clone()),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(gitdir),
            }
            .with_repo(repo))
        }
    }

    /// Discover the Git repository in `directory` and return it as context.
    pub fn discover(directory: impl AsRef<Path>) -> anyhow::Result<Context> {
        let directory = directory.as_ref();
        let repo = gix::discover(directory)?;
        #[cfg(feature = "legacy")]
        {
            use anyhow::Context as _;
            let worktree_dir = repo
                .workdir()
                .context("Bare repositories aren't yet supported.")?;
            let legacy_project = LegacyProject::find_by_worktree_dir(worktree_dir)
                .unwrap_or_else(|_| default_legacy_project_at_repo(&repo));
            let gitdir = repo.git_dir().to_owned();
            Ok(Context {
                settings: AppSettings::load_from_default_path_creating_without_customization()?,
                gitdir: gitdir.clone(),
                legacy_project,
                repo: new_ondemand_repo(gitdir.clone()),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(gitdir),
            }
            .with_repo(repo))
        }

        #[cfg(not(feature = "legacy"))]
        {
            let gitdir = repo.git_dir().to_owned();
            Ok(crate::Context {
                gitdir: gitdir.clone(),
                settings: AppSettings::load_from_default_path_creating_without_customization()?,
                repo: new_ondemand_repo(gitdir.clone()),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(gitdir),
            })
        }
    }

    /// Create a context that already has `repo` initialised and ready to be returned.
    ///
    /// Particularly useful in testing, which might start off with just a Git repository.
    /// **Note that it does not have support for legacy projects to encourage single-branch compatible code.**
    pub fn from_repo(repo: gix::Repository) -> anyhow::Result<Context> {
        let gitdir = repo.git_dir().to_owned();
        let settings = AppSettings::load_from_default_path_creating_without_customization()?;

        Ok(Context {
            #[cfg(feature = "legacy")]
            legacy_project: default_legacy_project_at_repo(&repo),
            gitdir: gitdir.clone(),
            settings,
            repo: new_ondemand_repo(gitdir.clone()),
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(gitdir),
        }
        .with_repo(repo))
    }

    /// Use `git2_repo` instead of the default repository that would be opened on first query.
    pub fn with_git2_repo(mut self, git2_repo: git2::Repository) -> Self {
        self.git2_repo.assign(git2_repo);
        self
    }

    /// Use `repo` instead of the default repository that would be opened on first query.
    pub fn with_repo(mut self, repo: gix::Repository) -> Self {
        self.repo.assign(repo);
        self
    }
}

/// Trampolines that create new uncached instances of major types.
impl Context {
    /// Open the repository with standard options and create a new Graph traversal from the current HEAD,
    /// along with a new metadata instance, and the graph itself.
    ///
    /// The write-permission is required to obtain a mutable metadata instance. Note that it must be held
    /// for until the end of the operation for the protection to be effective.
    ///
    /// Use [`Self::graph_and_meta_and_repo_from_head()`] if control over the repository configuration is needed.
    pub fn graph_and_meta_mut_and_repo_from_head(
        &self,
        _write: &mut WorktreeWritePermission,
    ) -> anyhow::Result<(
        gix::Repository,
        impl but_core::RefMetadata + 'static,
        but_graph::Graph,
    )> {
        let repo = self.repo.get()?;
        let meta = self.meta_inner()?;
        let graph = but_graph::Graph::from_head(&repo, &meta, but_graph::init::Options::limited())?;
        Ok((repo.clone(), meta, graph))
    }

    /// Create a new Graph traversal from the current HEAD, using (and returning) the given `repo` (configured by the caller),
    /// along with a new metadata instance, and the graph itself.
    ///
    /// The read-permission is required to obtain a shared metadata instance. Note that it must be held
    /// for until the end of the operation for the protection to be effective.
    pub fn graph_and_meta_and_repo_from_head(
        &self,
        repo: gix::Repository,
        _read_only: &WorktreeReadPermission,
    ) -> anyhow::Result<(
        gix::Repository,
        impl but_core::RefMetadata + 'static,
        but_graph::Graph,
    )> {
        let meta = self.meta_inner()?;
        let graph = but_graph::Graph::from_head(&repo, &meta, but_graph::init::Options::limited())?;
        Ok((repo, meta, graph))
    }

    fn meta_inner(&self) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }
}

/// Utilities
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
    ) -> anyhow::Result<impl but_core::RefMetadata + 'static> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }

    /// Copy all copyable values into an instance to pass across thread boundaries.
    pub fn to_sync(&self) -> ThreadSafeContext {
        ThreadSafeContext {
            settings: self.settings.clone(),
            gitdir: self.gitdir.clone(),
            repo: self.repo.get_opt().clone().map(|r| r.into_sync()),
            #[cfg(feature = "legacy")]
            legacy_project: self.legacy_project.clone(),
        }
    }

    /// Take all copyable values and place them in an instance that can pass across thread boundaries.
    pub fn into_sync(self) -> ThreadSafeContext {
        let Context {
            settings,
            gitdir,
            mut repo,
            git2_repo: _,
            db: _,
            #[cfg(feature = "legacy")]
            legacy_project,
        } = self;
        ThreadSafeContext {
            settings,
            gitdir,
            repo: repo.take().map(|r| r.into_sync()),
            #[cfg(feature = "legacy")]
            legacy_project,
        }
    }
}

impl ThreadSafeContext {
    /// Turn this instance back into a thread-local version, possibly keeping previously cached values.
    pub fn into_thread_local(self) -> Context {
        self.into()
    }
}

/// Paths and locations.
impl Context {
    /// The location where project-specific data can be stored that is owned by the application.
    pub fn project_data_dir(&self) -> PathBuf {
        project_data_dir(&self.gitdir)
    }

    /// Return the worktree directory associated with the context Git [repository](Self::repo).
    pub fn workdir(&self) -> anyhow::Result<Option<PathBuf>> {
        self.repo.get().map(|repo| repo.workdir().map(Into::into))
    }

    /// The path to the worktree directory or the `.git` directory if there is no worktree directory.
    /// Fallible as it may need to open a repository.
    pub fn workdir_or_gitdir(&self) -> anyhow::Result<PathBuf> {
        let repo = self.repo.get()?;
        Ok(repo.workdir().unwrap_or(repo.git_dir()).to_owned())
    }
}

/// Accessors
impl Context {
    /// Return a shared references to the application settings.
    ///
    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }
}

/// *Repository* helpers, for when you need something more specific than [Self::repo].
impl Context {
    /// Open an isolated repository, one that didn't read options beyond `.git/config` and
    /// knows no environment variables.
    ///
    /// Use it for fastest-possible access, when incomplete configuration is acceptable.
    /// Note that [Self::repo].get() should be preferred.
    pub fn open_isolated_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open_opts(
            &self.gitdir,
            gix::open::Options::isolated(),
        )?)
    }

    /// Return a cloned [`Repository`](gix::Repository) as cached in the context, with all configuration available
    /// to correctly figure out author and committer names (i.e. with most global configuration loaded),
    /// *and* which will perform diffs quickly thanks to an adequate object cache.
    ///
    /// This naturally is also useful for merging, but *only if these merged objects are supposed to be persisted immediately*.
    /// Note that the object cache will be temporary this way, and is dropped when the instance is dropped.
    pub fn clone_repo_for_merging(&self) -> anyhow::Result<gix::Repository> {
        let repo = self.repo.get()?.clone().for_tree_diffing()?;
        Ok(repo)
    }

    /// Return a cloned [`Repository`](gix::Repository) as cached in the context, with all configuration available
    /// to correctly figure out author and committer names (i.e. with most global configuration loaded),
    /// *and* which will perform diffs quickly thanks to an adequate object cache, *and*
    /// which **writes all objects into memory**.
    ///
    /// This means *changes are non-persisting*.
    /// Note that the object cache will be temporary this way, and is dropped when the instance is dropped.
    pub fn clone_repo_for_merging_non_persisting(&self) -> anyhow::Result<gix::Repository> {
        self.clone_repo_for_merging()
            .map(|repo| repo.with_object_memory())
    }
}

fn project_data_dir(gitdir: &Path) -> PathBuf {
    gitdir.join("gitbutler")
}

fn new_ondemand_repo(gitdir: PathBuf) -> OnDemand<gix::Repository> {
    OnDemand::new(move || gix::open(&gitdir).map_err(Into::into))
}

fn new_ondemand_git2_repo(gitdir: PathBuf) -> OnDemand<git2::Repository> {
    OnDemand::new({
        let gitdir = gitdir.clone();
        move || git2::Repository::open(&gitdir).map_err(Into::into)
    })
}

fn new_ondemand_db(gitdir: PathBuf) -> OnDemand<but_db::DbHandle> {
    OnDemand::new(move || but_db::DbHandle::new_in_directory(project_data_dir(&gitdir)))
}

#[cfg(feature = "legacy")]
fn default_legacy_project_at_repo(repo: &gix::Repository) -> LegacyProject {
    LegacyProject::default_with_id(LegacyProjectId::from_number_for_testing(1))
        .with_paths_for_testing(
            repo.git_dir().to_owned(),
            repo.workdir().map(ToOwned::to_owned),
        )
}

mod ondemand;
pub use ondemand::OnDemand;
