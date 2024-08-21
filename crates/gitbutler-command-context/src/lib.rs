use anyhow::Result;
use gitbutler_project::Project;

pub struct CommandContext {
    /// The git repository of the `project` itself.
    git_repository: git2::Repository,
    /// Metadata about the project, typically stored with GitButler application data.
    project: Project,
}

impl CommandContext {
    /// Open the repository identified by `project` and perform some checks.
    pub fn open(project: &Project) -> Result<Self> {
        let repo = git2::Repository::open(&project.path)?;

        // XXX(qix-): This is a temporary measure to disable GC on the project repository.
        // XXX(qix-): We do this because the internal repository we use to store the "virtual"
        // XXX(qix-): refs and information use Git's alternative-objects mechanism to refer
        // XXX(qix-): to the project repository's objects. However, the project repository
        // XXX(qix-): has no knowledge of these refs, and will GC them away (usually after
        // XXX(qix-): about 2 weeks) which will corrupt the internal repository.
        // XXX(qix-):
        // XXX(qix-): We will ultimately move away from an internal repository for a variety
        // XXX(qix-): of reasons, but for now, this is a simple, short-term solution that we
        // XXX(qix-): can clean up later on. We're aware this isn't ideal.
        if let Ok(config) = repo.config().as_mut() {
            let should_set = match config.get_bool("gitbutler.didSetPrune") {
                Ok(false) => true,
                Ok(true) => false,
                Err(err) => {
                    tracing::warn!(
                                "failed to get gitbutler.didSetPrune for repository at {}; cannot disable gc: {}",
                                project.path.display(),
                                err
                            );
                    false
                }
            };

            if should_set {
                if let Err(error) = config
                    .set_str("gc.pruneExpire", "never")
                    .and_then(|()| config.set_bool("gitbutler.didSetPrune", true))
                {
                    tracing::warn!(
                                "failed to set gc.auto to false for repository at {}; cannot disable gc: {}",
                                project.path.display(),
                                error
                            );
                }
            }
        } else {
            tracing::warn!(
                "failed to get config for repository at {}; cannot disable gc",
                project.path.display()
            );
        }

        Ok(Self {
            git_repository: repo,
            project: project.clone(),
        })
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Return the [`project`](Self::project) repository.
    pub fn repository(&self) -> &git2::Repository {
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
        Ok(gix::open(self.repository().path())?)
    }

    /// Return a newly opened `gitoxide` repository with only the repository-local configuration
    /// available. This is a little faster as it has to open less files upon startup.
    ///
    /// Such repositories are only useful for reference and object-access, but *can't be used* to create
    /// commits, fetch or push.
    pub fn gix_repository_minimal(&self) -> Result<gix::Repository> {
        Ok(gix::open_opts(
            self.repository().path(),
            gix::open::Options::isolated(),
        )?)
    }
}

// TODO(ST): put this into `gix`, the logic seems good, add unit-test for number generation.
pub trait GixRepositoryExt: Sized {
    /// Configure the repository for diff operations between trees.
    /// This means it needs an object cache relative to the amount of files in the repository.
    fn for_tree_diffing(self) -> Result<Self>;
}

impl GixRepositoryExt for gix::Repository {
    fn for_tree_diffing(mut self) -> anyhow::Result<Self> {
        let bytes = self.compute_object_cache_size_for_tree_diffs(&***self.index_or_empty()?);
        self.object_cache_size_if_unset(bytes);
        Ok(self)
    }
}
