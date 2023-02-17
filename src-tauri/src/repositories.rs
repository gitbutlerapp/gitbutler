use crate::{deltas, projects, sessions, users};
use anyhow::{Context, Result};
use std::collections::HashMap;

pub struct Repository {
    project: projects::Project,
    git_repository: git2::Repository,
}

impl Repository {
    pub fn open(
        projects_storage: &projects::Storage,
        users_storage: &users::Storage,
        repository_id: &str,
    ) -> Result<Self> {
        let project = projects_storage
            .get_project(repository_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", repository_id))?;
        let user = users_storage.get().context("failed to get user")?;
        let git_repository =
            git2::Repository::open(&project.path).context("failed to open repository")?;
        init(&git_repository, &project, &user)?;
        Ok(Repository {
            project,
            git_repository,
        })
    }

    fn reference(&self) -> Result<git2::Reference> {
        let reference_name = self.project.refname();
        let reference = self
            .git_repository
            .find_reference(&reference_name)
            .with_context(|| {
                format!(
                    "failed to find reference {} in repository {}",
                    reference_name, self.project.path
                )
            })?;
        Ok(reference)
    }

    pub fn sessions(&self) -> Result<Vec<sessions::Session>> {
        sessions::list(&self.git_repository, &self.project, &self.reference()?)
    }

    pub fn files(&self, session_id: &str) -> Result<HashMap<String, String>> {
        sessions::list_files(
            &self.git_repository,
            &self.project,
            &self.reference()?,
            session_id,
        )
    }

    pub fn deltas(&self, session_id: &str) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        deltas::list(
            &self.git_repository,
            &self.project,
            &self.reference()?,
            session_id,
        )
    }
}

fn init(
    git_repository: &git2::Repository,
    project: &projects::Project,
    user: &Option<users::User>,
) -> Result<()> {
    let reference_name = project.refname();
    match git_repository.find_reference(&reference_name) {
        // if the reference exists, we do nothing
        Ok(_) => Ok(()),
        // if the reference doesn't exist, we create it by creating a flushing a new session
        Err(error) => {
            if error.code() == git2::ErrorCode::NotFound {
                // if the reference doesn't exist, we create it by creating a flushing a new session
                let mut current_session = match sessions::Session::current(git_repository, project)?
                {
                    Some(session) => session,
                    None => sessions::Session::from_head(git_repository, project)?,
                };
                sessions::flush_session(git_repository, user, project, &mut current_session)?;
                Ok(())
            } else {
                Err(error).with_context(|| {
                    format!(
                        "failed to find reference {} in repository {}",
                        reference_name, project.path
                    )
                })
            }
        }
    }
}
