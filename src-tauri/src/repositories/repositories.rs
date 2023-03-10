use crate::{deltas, projects, sessions, users};
use anyhow::{Context, Result};
use std::collections::HashMap;

pub struct Repository {
    pub project: projects::Project,
    pub git_repository: git2::Repository,
}

impl Repository {
    pub fn open(
        projects_storage: &projects::Storage,
        users_storage: &users::Storage,
        project_id: &str,
    ) -> Result<Self> {
        let project = projects_storage
            .get_project(project_id)
            .with_context(|| "failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;
        let user = users_storage
            .get()
            .with_context(|| "failed to get user for project")?;
        let git_repository =
            git2::Repository::open(&project.path).with_context(|| "failed to open repository")?;
        Self::new(project, git_repository, user)
    }

    pub fn new(
        project: projects::Project,
        git_repository: git2::Repository,
        user: Option<users::User>,
    ) -> Result<Self> {
        init(&git_repository, &project, &user).with_context(|| "failed to init repository")?;
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

    pub fn files(
        &self,
        session_id: &str,
        files: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        sessions::list_files(
            &self.git_repository,
            &self.project,
            &self.reference()?,
            session_id,
            files,
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

    // get file status from git
    pub fn status(&self) -> HashMap<String, String> {
        println!("Git Status");

        let mut options = git2::StatusOptions::new();
        options.include_untracked(true);
        options.include_ignored(false);
        options.recurse_untracked_dirs(true);

        // get the status of the repository
        let statuses = self
            .git_repository
            .statuses(Some(&mut options))
            .with_context(|| "failed to get repository status");

        let mut files = HashMap::new();

        match statuses {
            Ok(statuses) => {
                // iterate over the statuses
                for entry in statuses.iter() {
                    // get the path of the entry
                    let path = entry.path().unwrap();
                    // get the status as a string
                    let istatus = match entry.status() {
                        s if s.contains(git2::Status::WT_NEW) => "added",
                        s if s.contains(git2::Status::WT_MODIFIED) => "modified",
                        s if s.contains(git2::Status::WT_DELETED) => "deleted",
                        s if s.contains(git2::Status::WT_RENAMED) => "renamed",
                        s if s.contains(git2::Status::WT_TYPECHANGE) => "typechange",
                        _ => continue,
                    };
                    files.insert(path.to_string(), istatus.to_string());
                }
            }
            Err(_) => {
                println!("Error getting status");
            }
        }

        return files;
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
                current_session
                    .flush(git_repository, user, project)
                    .with_context(|| format!("{}: failed to flush session", project.id))?;
                Ok(())
            } else {
                Err(error.into())
            }
        }
    }
}
