use anyhow::Result;
use but_settings::AppSettings;
use gitbutler_project::Project;
use std::path::Path;

pub struct CommandContext {
    /// The git repository of the `project` itself.
    git_repository: git2::Repository,
    /// Metadata about the project, typically stored with GitButler application data.
    project: Project,
    /// A snapshot of the app settings obtained at the beginnig of each command.
    app_settings: AppSettings,
}

impl CommandContext {
    /// Open the repository identified by `project` and perform some checks.
    pub fn open(project: &Project, app_settings: AppSettings) -> Result<Self> {
        let repo = git2::Repository::open(&project.path)?;
        Ok(Self {
            git_repository: repo,
            project: project.clone(),
            app_settings,
        })
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Return the [`project`](Self::project) repository.
    pub fn repo(&self) -> &git2::Repository {
        &self.git_repository
    }

    /// Return a newly opened `gitoxide` repository, with all configuration available
    /// to correctly figure out author and committer names (i.e. with most global configuration loaded).
    ///
    /// ### Note
    ///
    /// The plan is to eventually phase out the `git2` version of the repository, and open
    /// the `gitoxide` repository right away. Meanwhile, we open `gitoxide` repositories on the fly
    /// on top-level functions, and pass them down as needed.
    ///
    /// Also note that there are plenty of other places where repositories are opened ad-hoc, and
    /// there is no need to use this type there at all - opening a repo is very cheap still.
    pub fn gix_repository(&self) -> Result<gix::Repository> {
        Ok(gix::open(self.repo().path())?)
    }

    /// Return a newly opened `gitoxide` repository, with all configuration available
    /// to correctly figure out author and committer names (i.e. with most global configuration loaded),
    /// *and* which will perform diffs quickly thanks to an adequate object cache.
    pub fn gix_repository_for_merging(&self) -> Result<gix::Repository> {
        gix_repository_for_merging(self.repo().path())
    }

    /// Return a newly opened `gitoxide` repository, with all configuration available
    /// to correctly figure out author and committer names (i.e. with most global configuration loaded),
    /// *and* which will perform diffs quickly thanks to an adequate object cache, *and*
    /// which **writes all objects into memory**.
    ///
    /// This means *changes are non-persisting*.
    pub fn gix_repository_for_merging_non_persisting(&self) -> Result<gix::Repository> {
        Ok(self.gix_repository_for_merging()?.with_object_memory())
    }

    /// Return a newly opened `gitoxide` repository with only the repository-local configuration
    /// available. This is a little faster as it has to open less files upon startup.
    ///
    /// Such repositories are only useful for reference and object-access, but *can't be used* to create
    /// commits, fetch or push.
    pub fn gix_repository_minimal(&self) -> Result<gix::Repository> {
        Ok(gix::open_opts(
            self.repo().path(),
            gix::open::Options::isolated(),
        )?)
    }

    pub fn app_settings(&self) -> &AppSettings {
        &self.app_settings
    }
}

/// Return a newly opened `gitoxide` repository, with all configuration available
/// to correctly figure out author and committer names (i.e. with most global configuration loaded),
/// *and* which will perform diffs quickly thanks to an adequate object cache.
pub fn gix_repository_for_merging(worktree_or_git_dir: &Path) -> Result<gix::Repository> {
    let mut repo = gix::open(worktree_or_git_dir)?;
    let bytes = repo.compute_object_cache_size_for_tree_diffs(&***repo.index_or_empty()?);
    repo.object_cache_size_if_unset(bytes);
    Ok(repo)
}

mod repository_ext;
pub use repository_ext::RepositoryExtLite;
