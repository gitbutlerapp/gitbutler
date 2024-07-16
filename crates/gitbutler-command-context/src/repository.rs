use anyhow::{Context, Result};
use gitbutler_project::Project;
use itertools::Itertools;

pub struct ProjectRepository {
    git_repository: git2::Repository,
    project: Project,
}

impl ProjectRepository {
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

    pub fn set_project(&mut self, project: &Project) {
        self.project = project.clone();
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn repo(&self) -> &git2::Repository {
        &self.git_repository
    }

    /// Fetches a branches name without the remote name attached
    ///
    /// refs/heads/my-branch -> my-branch
    /// refs/remotes/origin/my-branch -> my-branch
    /// refs/remotes/Byron/gitbutler/my-branch -> my-branch (where the remote is Byron/gitbutler)
    ///
    /// An ideal implementation wouldn't require us to list all the references,
    /// but there doesn't seem to be a libgit2 solution to this.
    pub fn given_name_for_branch(&self, branch: &git2::Branch) -> Result<String> {
        let reference = branch.get();
        let repo = self.repo();

        if reference.is_remote() {
            let shorthand_name = reference
                .shorthand()
                .ok_or(anyhow::anyhow!("Branch name was not utf-8"))?;

            let remotes = repo.remotes().context("Failed to get remotes")?;

            let longest_remote = remotes
                .iter()
                .flatten()
                .sorted_by_key(|remote_name| -(remote_name.len() as i32))
                .find(|reference_name| shorthand_name.starts_with(reference_name))
                .ok_or(anyhow::anyhow!(
                    "Failed to find remote branch's corresponding remote"
                ))?;

            let shorthand_name = shorthand_name
                .strip_prefix(longest_remote)
                .and_then(|str| str.strip_prefix("/"))
                .ok_or(anyhow::anyhow!(
                    "Failed to cut remote name {} off of shorthand name {}",
                    longest_remote,
                    shorthand_name
                ))?;

            Ok(shorthand_name.to_string())
        } else {
            reference
                .shorthand()
                .ok_or(anyhow::anyhow!("Branch name was not utf-8"))
                .map(String::from)
        }
    }
}
