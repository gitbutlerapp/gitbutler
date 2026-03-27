//! A crate to host a `Context` type, suitable to provide the context *applications* need to operate on a Git repository.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::{
    cell,
    cell::RefCell,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use but_core::{
    RepositoryExt,
    sync::{RepoExclusive, RepoExclusiveGuard, RepoShared, RepoSharedGuard},
};
use but_error::Code;
use but_settings::AppSettings;
use tracing::instrument;

/// Legacy types that shouldn't be used.
#[cfg(feature = "legacy")]
pub mod legacy;
#[cfg(feature = "legacy")]
pub use legacy::types::{LegacyProject, LegacyProjectId};

/// Utilities to control access to the project directory of the context.
pub mod access;

/// Conversions from project identifier values into context types.
mod project_handle;
/// Project identifier types live in a temporary transition crate while legacy
/// project storage still exists.
///
/// Once `gitbutler-project` is dissolved, these types are expected to merge back into `but-ctx`.
/// Thus, use it through this crate only to simplify dependencies.
pub use but_project_handle::{ProjectHandle, ProjectHandleOrLegacyProjectId};

mod ondemand;
pub use ondemand::OnDemand;

mod ondemand_cache;
use crate::ondemand_cache::OnDemandCache;

/// A context specific to a repository, along with commonly used information to make higher-level functions
/// more convenient to implement.
/// This type is *not* thread-safe, and cheap to clone. That way it may own per-thread caches.
///
/// It's fine for it to one day receive thread-safe shared state, as needed, similar to [`gix::Repository`].
///
/// ### Keep it read-only if you can
///
/// Whenever something is mutable, either the database or the workspace, the `Context` used for interaction
/// must be mutable as well as `&mut Context`. Thus, in purely read-only situations, be sure to keep the `Context`
/// behind a shared reference as well as `&Context`.
///
/// ### DEADLOCK-ALERT: Beware of passing `ctx`: About Composability!
///
/// The callee *may* try to obtain their own locks which will deadlock if the caller is also holding
/// any lock. Don't for get to drop your own guards before making such calls.
///
/// Assume all `but_api` functions obtain a lock on their own.
///
/// Alternatively, design the callee to use [`RepoExclusive`] or [`RepoShared`] which
/// is automatically composable and deadlock free.
///
/// Locks may only be acquired by top-level callers, with permissions being passed down as needed.
/// Note that plumbing should not be forced to create permissions (which is inconvenient for testing),
/// and instead rely on the caller to know it's needed.
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
    /// It's also used for various derived directories to store GitButler data in.
    pub gitdir: PathBuf,
    /// The directory where per-project GitButler data is stored.
    ///
    /// Derived from [`gix::Repository::config_snapshot`] using a channel-specific
    /// Git config key (`gitbutler.storagePath` for release builds and
    /// `gitbutler.<channel>.storagePath` otherwise). Relative values are resolved
    /// against `gitdir`; paths that stay inside `gitdir` must live under a
    /// top-level directory whose name starts with `gitbutler`. Any resolved
    /// path outside `gitdir` gets a `<configured-path>/<project-handle>`
    /// suffix. If that key is not configured, the default is `gitdir/gitbutler`
    /// on all channels.
    pub project_data_dir: PathBuf,
    /// The directory to store application caches in.
    pub app_cache_dir: Option<PathBuf>,
    cache_mode: CacheMode,
    /// How repositories opened through this context should source their configuration.
    pub repo_open_mode: RepoOpenMode,
    /// The most recently opened repository of the project, which also provides access to the `git_dir`.
    ///
    /// # Tree-Diff optimization present
    ///
    /// Note that the standard repository comes with a decently sized object cache, but further optimization can
    /// be performed by using [`Self::clone_repo_for_merging()`].
    pub repo: OnDemand<gix::Repository>,
    /// The most recently opened `git2` repository of the project.
    pub git2_repo: OnDemand<git2::Repository>,
    /// An open handle to the database. It's initialized lazily upon first access.
    /// It is also what makes this type non-Clone, which is fair.
    pub db: OnDemand<but_db::DbHandle>,
    /// An open handle to the project-local cache, initialized lazily on first access and only fallible if it's already borrowed.
    pub cache: OnDemandCache<but_db::CacheHandle>,
    /// An open handle to the cache, initialized lazily on first access and only fallible if it's already borrowed.
    pub app_cache: OnDemandCache<but_db::AppCacheHandle>,
    /// The legacy implementation, for all the old code.
    #[cfg(feature = "legacy")]
    pub legacy_project: LegacyProject,

    /// A workspace based on any version of `repo`. It's expected to be kept up-to-date
    /// by anyone who changes it.
    /// It also can't be public as it needs several of our cached inputs first.
    workspace: RefCell<Option<but_graph::projection::Workspace>>,
}

/// A structure that can be passed across thread boundaries.
#[derive(Clone)]
pub struct ThreadSafeContext {
    /// The application context, here for convenience and as feature toggles and flags are needed.
    pub settings: AppSettings,
    /// The directory at which the repository itself is located.
    pub gitdir: PathBuf,
    /// The directory where per-project GitButler data is stored.
    pub project_data_dir: PathBuf,
    /// The directory to store application caches in.
    pub app_cache_dir: Option<PathBuf>,
    cache_mode: CacheMode,
    /// How repositories opened through this context should source their configuration.
    pub repo_open_mode: RepoOpenMode,
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
            project_data_dir,
            app_cache_dir,
            cache_mode,
            repo_open_mode,
            repo,
            #[cfg(feature = "legacy")]
            legacy_project,
        } = value;
        let mut ondemand = new_ondemand_repo(gitdir.clone(), repo_open_mode);
        if let Some(repo) = repo {
            ondemand.assign(repo.to_thread_local());
        }
        Context {
            settings,
            repo_open_mode,
            repo: ondemand,
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(project_data_dir.clone()),
            cache: new_ondemand_cache(project_data_dir.clone(), cache_mode),
            app_cache: new_ondemand_app_cache(app_cache_dir.clone(), cache_mode),
            gitdir,
            project_data_dir,
            app_cache_dir,
            cache_mode,
            #[cfg(feature = "legacy")]
            legacy_project,
            workspace: Default::default(),
        }
    }
}

impl TryFrom<gix::Repository> for Context {
    type Error = anyhow::Error;

    fn try_from(repo: gix::Repository) -> Result<Self, Self::Error> {
        Context::from_repo(repo)
    }
}

impl TryFrom<ProjectHandle> for Context {
    type Error = anyhow::Error;

    fn try_from(value: ProjectHandle) -> Result<Self, Self::Error> {
        Context::new_from_project_handle(value)
    }
}

impl TryFrom<ProjectHandle> for ThreadSafeContext {
    type Error = anyhow::Error;

    fn try_from(value: ProjectHandle) -> Result<Self, Self::Error> {
        let ctx: Context = value.try_into()?;
        Ok(ctx.into_sync())
    }
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("git_dir", &self.gitdir)
            .finish()
    }
}

impl std::fmt::Debug for ThreadSafeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadSafeContext")
            .field("git_dir", &self.gitdir)
            .finish()
    }
}

/// Control how `gix` repositories opened through a [`Context`] source configuration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RepoOpenMode {
    /// Use Git's normal config discovery, including user and environment-provided config.
    #[default]
    Standard,
    /// Read repository-local configuration only and ignore environment-provided config.
    Isolated,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CacheMode {
    #[default]
    Disk,
    Memory,
}

/// Lifecycle
impl Context {
    /// Create a new instance from just the `gitdir` of the repository we should provide context for.
    /// `app_config_dir` is where the application wide configuration lives.
    /// `app_cache_dir` is where application wide caches live. It's optional as caches are optional.
    pub fn new(
        gitdir: impl Into<PathBuf>,
        app_config_dir: impl AsRef<Path>,
        app_cache_dir: Option<PathBuf>,
    ) -> anyhow::Result<Context> {
        Self::new_with_repo_open_mode(
            gitdir,
            app_config_dir,
            app_cache_dir,
            RepoOpenMode::Standard,
        )
    }

    /// Just like [`Context::new()`], but allows controlling how the repository
    /// sources configuration via `repo_open_mode`.
    pub fn new_with_repo_open_mode(
        gitdir: impl Into<PathBuf>,
        app_config_dir: impl AsRef<Path>,
        app_cache_dir: Option<PathBuf>,
        repo_open_mode: RepoOpenMode,
    ) -> anyhow::Result<Context> {
        let gitdir = gitdir.into();
        let settings = app_settings(app_config_dir)?;
        let repo = open_repo(&gitdir, repo_open_mode)?;
        let project_data_dir = repo.gitbutler_storage_path()?;
        #[cfg(not(feature = "legacy"))]
        {
            Ok(Context {
                gitdir: gitdir.clone(),
                project_data_dir: project_data_dir.clone(),
                settings,
                cache_mode: CacheMode::Disk,
                repo_open_mode,
                repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(project_data_dir.clone()),
                cache: new_ondemand_cache(project_data_dir, CacheMode::Disk),
                app_cache: new_ondemand_app_cache(app_cache_dir.clone(), CacheMode::Disk),
                app_cache_dir,
                workspace: Default::default(),
            }
            .with_repo(repo))
        }
        #[cfg(feature = "legacy")]
        {
            use anyhow::Context as _;
            let worktree_dir = repo
                .workdir()
                .context("Bare repositories aren't yet supported.")?;
            let legacy_project = LegacyProject::find_by_worktree_dir(worktree_dir)
                .unwrap_or_else(|_| default_legacy_project_at_repo(&repo));
            let cache_mode = CacheMode::Disk;
            Ok(Context {
                settings,
                gitdir: gitdir.clone(),
                project_data_dir: project_data_dir.clone(),
                cache_mode,
                repo_open_mode,
                legacy_project,
                repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(project_data_dir.clone()),
                cache: new_ondemand_cache(project_data_dir, cache_mode),
                app_cache: new_ondemand_app_cache(app_cache_dir.clone(), cache_mode),
                app_cache_dir,
                workspace: Default::default(),
            }
            .with_repo(repo))
        }
    }

    /// Discover the Git repository in `directory`, or search upwards until one is found, and return it as context.
    pub fn discover(directory: impl AsRef<Path>) -> anyhow::Result<Context> {
        Self::discover_with_repo_open_mode(directory, RepoOpenMode::Standard)
    }

    /// Discover the Git repository in `directory`, or search upwards until one is found, and return it as context
    /// while controlling how the repository sources configuration via `repo_open_mode`.
    pub fn discover_with_repo_open_mode(
        directory: impl AsRef<Path>,
        repo_open_mode: RepoOpenMode,
    ) -> anyhow::Result<Context> {
        let directory = directory.as_ref();
        let gitdir = gix::discover(directory)?.git_dir().to_owned();
        let repo = open_repo(&gitdir, repo_open_mode)?;
        Self::from_repo_with_legacy_support(repo, repo_open_mode)
    }

    /// Create a context from a `project_handle`, which directly encodes the repository path.
    pub fn new_from_project_handle(project_handle: ProjectHandle) -> anyhow::Result<Self> {
        Self::new_from_project_handle_with_repo_open_mode(project_handle, RepoOpenMode::Standard)
    }

    /// Create a context from a `project_handle`, which directly encodes the repository path,
    /// while controlling how the repository sources configuration via `repo_open_mode`.
    pub fn new_from_project_handle_with_repo_open_mode(
        project_handle: ProjectHandle,
        repo_open_mode: RepoOpenMode,
    ) -> anyhow::Result<Self> {
        let repository_path = project_handle.into_path()?;
        let repo = open_repo(&repository_path, repo_open_mode)?;
        Self::from_repo_with_legacy_support(repo, repo_open_mode)
    }

    /// Open the Git repository in `directory` and return it as context.
    pub fn open(directory: impl AsRef<Path>) -> anyhow::Result<Context> {
        Self::open_with_repo_open_mode(directory, RepoOpenMode::Standard)
    }

    /// Open the Git repository in `directory` and return it as context while controlling how
    /// the repository sources configuration via `repo_open_mode`.
    pub fn open_with_repo_open_mode(
        directory: impl AsRef<Path>,
        repo_open_mode: RepoOpenMode,
    ) -> anyhow::Result<Context> {
        let directory = directory.as_ref();
        let repo = open_repo(directory, repo_open_mode)?;
        Self::from_repo_with_legacy_support(repo, repo_open_mode)
    }

    fn from_repo_with_legacy_support(
        repo: gix::Repository,
        repo_open_mode: RepoOpenMode,
    ) -> anyhow::Result<Context> {
        let app_cache_dir = but_path::app_cache_dir().ok();
        let project_data_dir = repo.gitbutler_storage_path()?;
        #[cfg(feature = "legacy")]
        {
            use anyhow::Context as _;
            let worktree_dir = repo
                .workdir()
                .context("Bare repositories aren't yet supported.")?;
            let legacy_project = LegacyProject::find_by_worktree_dir(worktree_dir)
                .unwrap_or_else(|_| default_legacy_project_at_repo(&repo));
            let gitdir = repo.git_dir().to_owned();
            let cache_mode = CacheMode::Disk;
            Ok(Context {
                settings: app_settings(but_path::app_config_dir()?)?,
                gitdir: gitdir.clone(),
                project_data_dir: project_data_dir.clone(),
                cache_mode,
                repo_open_mode,
                legacy_project,
                repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(project_data_dir.clone()),
                cache: new_ondemand_cache(project_data_dir, cache_mode),
                app_cache: new_ondemand_app_cache(app_cache_dir.clone(), cache_mode),
                app_cache_dir,
                workspace: Default::default(),
            }
            .with_repo(repo))
        }

        #[cfg(not(feature = "legacy"))]
        {
            let gitdir = repo.git_dir().to_owned();
            let cache_mode = CacheMode::Disk;
            Ok(crate::Context {
                gitdir: gitdir.clone(),
                project_data_dir: project_data_dir.clone(),
                settings: app_settings(but_path::app_config_dir()?)?,
                cache_mode,
                repo_open_mode,
                repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
                git2_repo: new_ondemand_git2_repo(gitdir.clone()),
                db: new_ondemand_db(project_data_dir.clone()),
                cache: new_ondemand_cache(project_data_dir, cache_mode),
                app_cache: new_ondemand_app_cache(app_cache_dir.clone(), cache_mode),
                app_cache_dir,
                workspace: Default::default(),
            }
            .with_repo(repo))
        }
    }

    /// Create a context that already has `repo` initialised and ready to be returned.
    ///
    /// Particularly useful in testing, which might start off with just a Git repository.
    /// **Note that it does not have support for legacy projects to encourage single-branch compatible code.**
    pub fn from_repo(repo: gix::Repository) -> anyhow::Result<Context> {
        let gitdir = repo.git_dir().to_owned();
        let project_data_dir = repo.gitbutler_storage_path()?;
        let settings = app_settings(but_path::app_config_dir()?)?;
        let app_cache_dir = but_path::app_cache_dir().ok();
        let repo_open_mode =
            if repo.open_options().permissions == gix::open::Permissions::isolated() {
                RepoOpenMode::Isolated
            } else {
                RepoOpenMode::Standard
            };

        let cache_mode = CacheMode::Disk;
        Ok(Context {
            #[cfg(feature = "legacy")]
            legacy_project: default_legacy_project_at_repo(&repo),
            gitdir: gitdir.clone(),
            project_data_dir: project_data_dir.clone(),
            settings,
            cache_mode,
            repo_open_mode,
            repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(project_data_dir.clone()),
            cache: new_ondemand_cache(project_data_dir, cache_mode),
            app_cache: new_ondemand_app_cache(app_cache_dir.clone(), cache_mode),
            app_cache_dir,
            workspace: Default::default(),
        }
        .with_repo(repo))
    }

    /// Use `repo` instead of the default repository that would be opened on first query.
    pub fn with_repo(mut self, repo: gix::Repository) -> Self {
        self.repo.assign(repo);
        self
    }

    /// Use in-memory caches instead of project/app cache files.
    ///
    /// This is useful for read-only contexts so cache access doesn't create SQLite files on disk.
    /// Prefer calling it before the first cache access.
    pub fn with_memory_cache(mut self) -> Self {
        self.cache_mode = CacheMode::Memory;
        self.cache = new_ondemand_cache(self.project_data_dir.clone(), self.cache_mode);
        self.app_cache = new_ondemand_app_cache(self.app_cache_dir.clone(), self.cache_mode);
        self
    }
}

/// Trampolines that create new uncached instances of major types.
impl Context {
    /// Create a cached workspace as seen from the current HEAD for editing, and return it,
    /// along with `(&repo, &mut ws, &mut db)`.
    /// `perm` ensures exclusive process-wide access to the repository.
    /// Once the repository is changed, the cached workspace should be updated.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write the new workspace back into `&mut ws`.
    #[instrument(name = "Context::workspace_mut_and_db_mut", level = "debug", skip_all)]
    #[expect(clippy::type_complexity)]
    pub fn workspace_mut_and_db_mut(
        &mut self,
    ) -> anyhow::Result<(
        RepoExclusiveGuard,
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::RefMut<'_, but_db::DbHandle>,
    )> {
        let mut guard = self.exclusive_worktree_access();
        let (repo, ws, db) = self.workspace_mut_and_db_mut_with_perm(guard.write_permission())?;
        Ok((guard, repo, ws, db))
    }

    /// Create a cached workspace as seen from the current HEAD for editing, and return it,
    /// along with `(&repo, &mut ws, &mut db, &cache)`.
    /// `perm` ensures exclusive process-wide access to the repository.
    /// Once the repository is changed, the cached workspace should be updated.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write the new workspace back into `&mut ws`.
    #[instrument(
        name = "Context::workspace_mut_and_db_mut_and_cache",
        level = "debug",
        skip_all
    )]
    #[expect(clippy::type_complexity)]
    pub fn workspace_mut_and_db_mut_and_cache(
        &mut self,
    ) -> anyhow::Result<(
        RepoExclusiveGuard,
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::RefMut<'_, but_db::DbHandle>,
        cell::Ref<'_, but_db::CacheHandle>,
    )> {
        let mut guard = self.exclusive_worktree_access();
        let (repo, ws, db, cache) =
            self.workspace_mut_and_db_mut_and_cache_with_perm(guard.write_permission())?;
        Ok((guard, repo, ws, db, cache))
    }

    /// Create a cached workspace as seen from the current HEAD for editing, and return it,
    /// along with `(&repo, &mut ws, &mut db)`.
    /// `perm` ensures exclusive process-wide access to the repository.
    /// Once the repository is changed, the cached workspace should be updated.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(
        name = "Context::workspace_mut_and_db_mut_with_perm",
        level = "debug",
        skip_all
    )]
    pub fn workspace_mut_and_db_mut_with_perm(
        &mut self,
        _perm: &mut RepoExclusive,
    ) -> anyhow::Result<(
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::RefMut<'_, but_db::DbHandle>,
    )> {
        let repo = self.repo.get()?;
        if let Ok(cached) =
            cell::RefMut::filter_map(self.workspace.try_borrow_mut()?, |opt| opt.as_mut())
        {
            let db = self.db.get_mut()?;
            return Ok((repo, cached, db));
        }
        let ws = self.workspace_from_head()?;
        {
            let mut value = self.workspace.try_borrow_mut()?;
            *value = Some(ws);
        }
        let ws = cell::RefMut::filter_map(self.workspace.borrow_mut(), |opt| opt.as_mut())
            .unwrap_or_else(|_| unreachable!("just set the value"));
        let db = self.db.get_mut()?;
        Ok((repo, ws, db))
    }

    /// Create a cached workspace as seen from the current HEAD for editing, and return it,
    /// along with `(&repo, &mut ws, &mut db, &cache)`.
    /// `perm` ensures exclusive process-wide access to the repository.
    /// Once the repository is changed, the cached workspace should be updated.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(
        name = "Context::workspace_mut_and_db_mut_and_cache_with_perm",
        level = "debug",
        skip_all
    )]
    #[expect(clippy::type_complexity)]
    pub fn workspace_mut_and_db_mut_and_cache_with_perm(
        &mut self,
        _perm: &mut RepoExclusive,
    ) -> anyhow::Result<(
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::RefMut<'_, but_db::DbHandle>,
        cell::Ref<'_, but_db::CacheHandle>,
    )> {
        let cache = self.cache.get_cache()?;
        let repo = self.repo.get()?;
        if let Ok(cached) =
            cell::RefMut::filter_map(self.workspace.try_borrow_mut()?, |opt| opt.as_mut())
        {
            let db = self.db.get_mut()?;
            return Ok((repo, cached, db, cache));
        }
        let ws = self.workspace_from_head()?;
        {
            let mut value = self.workspace.try_borrow_mut()?;
            *value = Some(ws);
        }
        let ws = cell::RefMut::filter_map(self.workspace.borrow_mut(), |opt| opt.as_mut())
            .unwrap_or_else(|_| unreachable!("just set the value"));
        let db = self.db.get_mut()?;
        Ok((repo, ws, db, cache))
    }

    /// Create a new cached workspace as seen from the current HEAD for *reading* and return it,
    /// along with `(guard, &repo, &mut ws, &mut db)`.
    /// The `db` is writable as this is more useful and naturally synced.
    /// The guard is for shared access to the repository.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(name = "Context::workspace_and_db_mut", level = "debug", skip_all)]
    #[expect(clippy::type_complexity)]
    pub fn workspace_and_db_mut(
        &mut self,
    ) -> anyhow::Result<(
        RepoSharedGuard,
        cell::Ref<'_, gix::Repository>,
        cell::Ref<'_, but_graph::projection::Workspace>,
        cell::RefMut<'_, but_db::DbHandle>,
    )> {
        let guard = self.shared_worktree_access();
        let (repo, ws, db) = self.workspace_and_db_mut_with_perm(guard.read_permission())?;
        Ok((guard, repo, ws, db))
    }

    /// Create a new cached workspace as seen from the current HEAD for *reading* and return it,
    /// along with `(guard, &repo, &mut ws, &mut db)`, given a read-`perm`ission.
    /// The `db` is writable as this is more useful and naturally synced.
    /// The guard is for shared access to the repository.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(
        name = "Context::workspace_and_db_mut_with_perm",
        level = "debug",
        skip_all
    )]
    pub fn workspace_and_db_mut_with_perm(
        &mut self,
        _perm: &RepoShared,
    ) -> anyhow::Result<(
        cell::Ref<'_, gix::Repository>,
        cell::Ref<'_, but_graph::projection::Workspace>,
        cell::RefMut<'_, but_db::DbHandle>,
    )> {
        if let Ok(cached) = cell::Ref::filter_map(self.workspace.try_borrow()?, |opt| opt.as_ref())
        {
            return Ok((self.repo.get()?, cached, self.db.get_mut()?));
        }
        let ws = self.workspace_from_head()?;
        {
            let mut value = self.workspace.try_borrow_mut()?;
            *value = Some(ws);
        }
        let ws = cell::Ref::filter_map(self.workspace.borrow(), |opt| opt.as_ref())
            .unwrap_or_else(|_| unreachable!("just set the value"));
        Ok((self.repo.get()?, ws, self.db.get_mut()?))
    }

    /// Create a new cached workspace as seen from the current HEAD for *writing* and return it,
    /// along with `(guard, &repo, &mut ws, &db)`.
    /// The `db` is read-only.
    /// The guard is for exclusive access to the repository.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(name = "Context::workspace_mut_from_head", level = "debug", skip_all)]
    #[expect(clippy::type_complexity)]
    pub fn workspace_mut_and_db(
        &mut self,
    ) -> anyhow::Result<(
        RepoExclusiveGuard,
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
    )> {
        let mut guard = self.exclusive_worktree_access();
        let (repo, ws, db) = self.workspace_mut_and_db_with_perm(guard.write_permission())?;
        Ok((guard, repo, ws, db))
    }

    /// Create a new cached workspace as seen from the current HEAD for *writing* and return it,
    /// along with `(guard, &repo, &mut ws, &db, &cache)`.
    /// The `db` and `cache` are read-only.
    /// The guard is for exclusive access to the repository.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(
        name = "Context::workspace_mut_and_db_and_cache",
        level = "debug",
        skip_all
    )]
    #[expect(clippy::type_complexity)]
    pub fn workspace_mut_and_db_and_cache(
        &mut self,
    ) -> anyhow::Result<(
        RepoExclusiveGuard,
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
        cell::Ref<'_, but_db::CacheHandle>,
    )> {
        let mut guard = self.exclusive_worktree_access();
        let (repo, ws, db, cache) =
            self.workspace_mut_and_db_and_cache_with_perm(guard.write_permission())?;
        Ok((guard, repo, ws, db, cache))
    }

    /// Create a new cached workspace as seen from the current HEAD for *reading* and return it,
    /// along with `(guard, &repo, &mut ws, &db)`, given a read-`perm`ission.
    /// The `db` is read-only.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    #[instrument(
        name = "Context::workspace_mut_and_db_with_perm",
        level = "debug",
        skip_all
    )]
    pub fn workspace_mut_and_db_with_perm(
        &self,
        _perm: &RepoExclusive,
    ) -> anyhow::Result<(
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
    )> {
        if let Ok(cached) =
            cell::RefMut::filter_map(self.workspace.try_borrow_mut()?, |opt| opt.as_mut())
        {
            return Ok((self.repo.get()?, cached, self.db.get()?));
        }
        let ws = self.workspace_from_head()?;
        {
            let mut value = self.workspace.try_borrow_mut()?;
            *value = Some(ws);
        }
        let ws = cell::RefMut::filter_map(self.workspace.borrow_mut(), |opt| opt.as_mut())
            .unwrap_or_else(|_| unreachable!("just set the value"));
        Ok((self.repo.get()?, ws, self.db.get()?))
    }

    /// Create a new cached workspace as seen from the current HEAD for *writing* and return it,
    /// along with `(&repo, &mut ws, &db, &cache)`, given a write-`perm`ission.
    /// The `db` and `cache` are read-only.
    ///
    /// # IMPORTANT
    /// * if the workspace was changed, write it back into `&mut ws`.
    #[instrument(
        name = "Context::workspace_mut_and_db_and_cache_with_perm",
        level = "debug",
        skip_all
    )]
    #[expect(clippy::type_complexity)]
    pub fn workspace_mut_and_db_and_cache_with_perm(
        &self,
        perm: &RepoExclusive,
    ) -> anyhow::Result<(
        cell::Ref<'_, gix::Repository>,
        cell::RefMut<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
        cell::Ref<'_, but_db::CacheHandle>,
    )> {
        let (repo, ws, db) = self.workspace_mut_and_db_with_perm(perm)?;
        let cache = self.cache.get_cache()?;
        Ok((repo, ws, db, cache))
    }

    /// Create a new cached workspace as seen from the current HEAD for *reading* and return it,
    /// along with `(guard, &repo, &ws, &db)`.
    /// The `db` is read-only.
    /// The guard is for shared access to the repository.
    ///
    /// # IMPORTANT
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(name = "Context::workspace_from_head", level = "debug", skip_all)]
    #[expect(clippy::type_complexity)]
    pub fn workspace_and_db(
        &self,
    ) -> anyhow::Result<(
        RepoSharedGuard,
        cell::Ref<'_, gix::Repository>,
        cell::Ref<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
    )> {
        let guard = self.shared_worktree_access();
        let (repo, ws, db) = self.workspace_and_db_with_perm(guard.read_permission())?;
        Ok((guard, repo, ws, db))
    }

    /// Create a new cached workspace as seen from the current HEAD for *reading* and return it,
    /// along with `(guard, &repo, &ws, &db, &cache)`.
    /// The `db` and `cache` are read-only.
    /// The guard is for shared access to the repository.
    ///
    /// # IMPORTANT
    /// * Keep the guard alive like `let (_guard, …) = …`!
    #[instrument(
        name = "Context::workspace_and_db_and_cache",
        level = "debug",
        skip_all
    )]
    #[expect(clippy::type_complexity)]
    pub fn workspace_and_db_and_cache(
        &self,
    ) -> anyhow::Result<(
        RepoSharedGuard,
        cell::Ref<'_, gix::Repository>,
        cell::Ref<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
        cell::Ref<'_, but_db::CacheHandle>,
    )> {
        let guard = self.shared_worktree_access();
        let (repo, ws, db, cache) =
            self.workspace_and_db_and_cache_with_perm(guard.read_permission())?;
        Ok((guard, repo, ws, db, cache))
    }

    /// Create a new cached workspace as seen from the current HEAD for *reading* and return it,
    /// along with `(guard, &repo, &ws, &db)`, given a read-`perm`ission.
    /// The `db` is read-only.
    #[instrument(
        name = "Context::workspace_and_db_with_perm",
        level = "debug",
        skip_all
    )]
    pub fn workspace_and_db_with_perm(
        &self,
        _perm: &RepoShared,
    ) -> anyhow::Result<(
        cell::Ref<'_, gix::Repository>,
        cell::Ref<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
    )> {
        if let Ok(cached) = cell::Ref::filter_map(self.workspace.try_borrow()?, |opt| opt.as_ref())
        {
            return Ok((self.repo.get()?, cached, self.db.get()?));
        }
        let ws = self.workspace_from_head()?;
        {
            let mut value = self.workspace.try_borrow_mut()?;
            *value = Some(ws);
        }
        let ws = cell::Ref::filter_map(self.workspace.borrow(), |opt| opt.as_ref())
            .unwrap_or_else(|_| unreachable!("just set the value"));
        Ok((self.repo.get()?, ws, self.db.get()?))
    }

    /// Create a new cached workspace as seen from the current HEAD for *reading* and return it,
    /// along with `(&repo, &ws, &db, &cache)`, given a read-`perm`ission.
    /// The `db` and `cache` are read-only.
    #[instrument(
        name = "Context::workspace_and_db_and_cache_with_perm",
        level = "debug",
        skip_all
    )]
    #[expect(clippy::type_complexity)]
    pub fn workspace_and_db_and_cache_with_perm(
        &self,
        perm: &RepoShared,
    ) -> anyhow::Result<(
        cell::Ref<'_, gix::Repository>,
        cell::Ref<'_, but_graph::projection::Workspace>,
        cell::Ref<'_, but_db::DbHandle>,
        cell::Ref<'_, but_db::CacheHandle>,
    )> {
        let (repo, ws, db) = self.workspace_and_db_with_perm(perm)?;
        let cache = self.cache.get_cache()?;
        Ok((repo, ws, db, cache))
    }

    fn workspace_from_head(&self) -> anyhow::Result<but_graph::projection::Workspace> {
        let repo = self.repo.get()?;
        let meta = self.meta_inner()?;
        let graph = but_graph::Graph::from_head(&repo, &meta, but_graph::init::Options::limited())?;
        graph.into_workspace()
    }

    fn meta_inner(&self) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }
}

/// Non-project/global data access. Only for when no project is available (yet).
impl Context {
    /// Create an application cache handle in the configured application cache directory.
    /// This is independent of projects.
    ///
    /// This is a convenience for callers that need app-cache access before a `Context`
    /// instance is available. If you have an instance, use [`Self::app_cache`] instead.
    pub fn app_cache() -> but_db::AppCacheHandle {
        but_db::AppCacheHandle::new_in_directory(but_path::app_cache_dir().ok())
    }
}

/// Utilities
impl Context {
    /// Return a wrapper for metadata that only supports read-only access when presented with the project wide permission
    /// to read data.
    /// This is helping to prevent races with mutable instances.
    // TODO(ctx): remove method entirely as we don't need it anymore with a DB
    //            based implementation as long as the instances starts a transaction to isolate
    //            reads. For a correct implementation, this would also have to hold on to
    //            `_read_only`.
    pub fn meta(&self) -> anyhow::Result<impl but_core::RefMetadata + 'static> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }

    /// Copy all copyable values into an instance to pass across thread boundaries.
    pub fn to_sync(&self) -> ThreadSafeContext {
        ThreadSafeContext {
            settings: self.settings.clone(),
            gitdir: self.gitdir.clone(),
            project_data_dir: self.project_data_dir.clone(),
            app_cache_dir: self.app_cache_dir.clone(),
            cache_mode: self.cache_mode,
            repo_open_mode: self.repo_open_mode,
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
            project_data_dir,
            mut repo,
            git2_repo: _,
            db: _,
            cache: _,
            app_cache: _,
            app_cache_dir,
            cache_mode,
            repo_open_mode,
            #[cfg(feature = "legacy")]
            legacy_project,
            workspace: _,
        } = self;
        ThreadSafeContext {
            settings,
            gitdir,
            project_data_dir,
            app_cache_dir,
            cache_mode,
            repo_open_mode,
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
        self.project_data_dir.clone()
    }

    /// The path to the worktree directory or the `.git` directory if there is no worktree directory.
    /// Fallible as it may need to open a repository.
    pub fn workdir_or_gitdir(&self) -> anyhow::Result<PathBuf> {
        let repo = self.repo.get()?;
        Ok(repo.workdir().unwrap_or(repo.git_dir()).to_owned())
    }

    /// Return the worktree directory associated with the context Git [repository](Self::repo).
    pub fn workdir(&self) -> anyhow::Result<Option<PathBuf>> {
        self.repo.get().map(|repo| repo.workdir().map(Into::into))
    }

    /// Return the worktree directory associated with the context Git [repository](Self::repo),
    /// or fail.
    ///
    /// # Try to gracefully degrade if there is no worktree!
    pub fn workdir_or_fail(&self) -> anyhow::Result<PathBuf> {
        let repo = self.repo.get()?;
        repo.workdir()
            .ok_or_else(|| anyhow!("Cannot currently work in repositories without a worktree"))
            .map(Into::into)
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

    /// Eagerly open the legacy `git2` repository, preserving repo-ownership error tagging used
    /// by activation entrypoints before any database work begins.
    ///
    /// This is needed as parts of the code depend on the error carrying this context code
    /// to indicate ownership issues, related to `safe.directories` or the lack thereof.
    /// `gix::Repository` doesn't have that problem as it functions differently,
    /// not by refusing to open the repository but by not trusting its configuration in particular when
    /// programs are involved.
    pub fn eagerly_populate_git2_repo_cache(&mut self) -> anyhow::Result<()> {
        {
            let existing = self.git2_repo.get_opt();
            if existing.is_some() {
                return Ok(());
            }
        }
        let repo = git2::Repository::open(&self.gitdir).map_err(|err| {
            let code = err.code();
            let err = anyhow::Error::from(err);
            if code == git2::ErrorCode::Owner {
                err.context(Code::RepoOwnership)
            } else {
                err
            }
        })?;
        self.git2_repo.assign(repo);
        Ok(())
    }
}

/// For now, always make sure we have object caches setup to make diffs fast in the common case.
/// Optimizing this based on better heuristics can be done with [Context::clone_repo_for_merging()].
fn new_ondemand_repo(gitdir: PathBuf, repo_open_mode: RepoOpenMode) -> OnDemand<gix::Repository> {
    OnDemand::new(move || {
        open_repo(&gitdir, repo_open_mode).map(|mut repo| {
            repo.object_cache_size_if_unset(100 * 1024 * 1024);
            repo
        })
    })
}

fn open_repo(gitdir: &Path, repo_open_mode: RepoOpenMode) -> anyhow::Result<gix::Repository> {
    match repo_open_mode {
        RepoOpenMode::Standard => Ok(gix::open(gitdir)?),
        RepoOpenMode::Isolated => Ok(gix::open_opts(gitdir, gix::open::Options::isolated())?),
    }
}

#[instrument(level = "trace")]
fn new_ondemand_git2_repo(gitdir: PathBuf) -> OnDemand<git2::Repository> {
    OnDemand::new({
        let gitdir = gitdir.clone();
        move || git2::Repository::open(&gitdir).map_err(Into::into)
    })
}

#[instrument(level = "trace")]
fn new_ondemand_db(project_data_dir: PathBuf) -> OnDemand<but_db::DbHandle> {
    OnDemand::new(move || but_db::DbHandle::new_in_directory(project_data_dir.clone()))
}

#[instrument(level = "trace")]
fn new_ondemand_cache(
    project_data_dir: PathBuf,
    cache_mode: CacheMode,
) -> OnDemandCache<but_db::CacheHandle> {
    OnDemandCache::new(move || match cache_mode {
        CacheMode::Disk => but_db::CacheHandle::new_in_directory(project_data_dir.clone()),
        CacheMode::Memory => but_db::CacheHandle::new_at_path(":memory:"),
    })
}

#[instrument(level = "trace")]
fn new_ondemand_app_cache(
    cache_dir: Option<PathBuf>,
    cache_mode: CacheMode,
) -> OnDemandCache<but_db::AppCacheHandle> {
    OnDemandCache::new(move || match cache_mode {
        CacheMode::Disk => but_db::AppCacheHandle::new_in_directory(cache_dir.clone()),
        CacheMode::Memory => but_db::AppCacheHandle::new_at_path(":memory:"),
    })
}

fn app_settings(config_dir: impl AsRef<Path>) -> anyhow::Result<AppSettings> {
    AppSettings::load(
        &AppSettings::default_settings_path(config_dir.as_ref()),
        None,
    )
}

#[cfg(feature = "legacy")]
fn default_legacy_project_at_repo(repo: &gix::Repository) -> LegacyProject {
    LegacyProject::default_with_id(ProjectHandleOrLegacyProjectId::LegacyProjectId(
        LegacyProjectId::from_number_for_testing(1),
    ))
    .with_paths_for_testing(
        repo.git_dir().to_owned(),
        repo.workdir().map(ToOwned::to_owned),
    )
}
