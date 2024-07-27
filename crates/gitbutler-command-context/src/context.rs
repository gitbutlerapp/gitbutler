use anyhow::{bail, Context, Result};
use gitbutler_project::Project;

trait RepositoryExt {
    fn on_integration_branch(&self) -> Result<bool>;
}

impl RepositoryExt for git2::Repository {
    /// Determines whether the user is currently on the gitbutler/integration reference
    fn on_integration_branch(&self) -> Result<bool> {
        let head_ref = self.head().context("failed to get head")?;
        let head_ref_name = head_ref.name().context("failed to get head name")?;
        Ok(head_ref_name != "refs/heads/gitbutler/integration")
    }
}

#[allow(dead_code)]
pub enum RequestContext {
    OpenWorkspace(OpenWorkspaceContext),
    OutsideWorkspace(OutsideWorkspaceContext),
}

#[allow(dead_code)]
pub struct OutsideWorkspaceContext {
    git_repository: git2::Repository,
    project: Project,
}

pub struct OpenWorkspaceContext {
    git_repository: git2::Repository,
    project: Project,
}

impl RequestContext {
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

        if repo.on_integration_branch()? {
            Ok(RequestContext::OpenWorkspace(OpenWorkspaceContext {
                git_repository: repo,
                project: project.clone(),
            }))
        } else {
            Ok(RequestContext::OutsideWorkspace(OutsideWorkspaceContext {
                git_repository: repo,
                project: project.clone(),
            }))
        }
    }

    pub fn try_create_open_workspace_context(project: &Project) -> Result<OpenWorkspaceContext> {
        let RequestContext::OpenWorkspace(open_workspace_context) = RequestContext::open(project)?
        else {
            bail!("Open workspace required for this action");
        };

        Ok(open_workspace_context)
    }
}

pub trait ContextProjectAccess {
    fn project(&self) -> &Project;
    fn set_project(&mut self, project: &Project);
}

pub trait ContextRepositoryAccess {
    fn repo(&self) -> &git2::Repository;
}

impl ContextProjectAccess for OpenWorkspaceContext {
    fn set_project(&mut self, project: &Project) {
        self.project = project.clone();
    }

    fn project(&self) -> &Project {
        &self.project
    }
}

impl ContextRepositoryAccess for OpenWorkspaceContext {
    fn repo(&self) -> &git2::Repository {
        &self.git_repository
    }
}

impl ContextProjectAccess for OutsideWorkspaceContext {
    fn set_project(&mut self, project: &Project) {
        self.project = project.clone();
    }

    fn project(&self) -> &Project {
        &self.project
    }
}

impl ContextRepositoryAccess for OutsideWorkspaceContext {
    fn repo(&self) -> &git2::Repository {
        &self.git_repository
    }
}

impl ContextProjectAccess for RequestContext {
    fn set_project(&mut self, project: &Project) {
        match self {
            Self::OpenWorkspace(open_workspace_context) => {
                open_workspace_context.set_project(project)
            }
            Self::OutsideWorkspace(outside_workspace_context) => {
                outside_workspace_context.set_project(project)
            }
        }
    }

    fn project(&self) -> &Project {
        match self {
            Self::OpenWorkspace(open_workspace_context) => open_workspace_context.project(),
            Self::OutsideWorkspace(outside_workspace_context) => {
                outside_workspace_context.project()
            }
        }
    }
}

impl ContextRepositoryAccess for RequestContext {
    fn repo(&self) -> &git2::Repository {
        match self {
            Self::OpenWorkspace(open_workspace_context) => open_workspace_context.repo(),
            Self::OutsideWorkspace(outside_workspace_context) => outside_workspace_context.repo(),
        }
    }
}
