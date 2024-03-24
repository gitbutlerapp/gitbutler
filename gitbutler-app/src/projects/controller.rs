use super::{storage, storage::UpdateRequest, Project, ProjectId};
use crate::{gb_repository, project_repository, users, watcher};
use anyhow::Context;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: PathBuf,
    projects_storage: storage::Storage,
    users: users::Controller,
    watchers: Option<watcher::Watchers>,
}

impl Controller {
    pub fn new(
        local_data_dir: PathBuf,
        projects_storage: storage::Storage,
        users: users::Controller,
        watchers: Option<watcher::Watchers>,
    ) -> Self {
        Self {
            local_data_dir,
            projects_storage,
            users,
            watchers,
        }
    }

    #[cfg(test)]
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Self {
        let pathbuf = path.as_ref().to_path_buf();
        Self {
            local_data_dir: pathbuf.clone(),
            projects_storage: storage::Storage::from_path(&pathbuf),
            users: users::Controller::from_path(&pathbuf),
            watchers: None,
        }
    }

    pub fn add<P: AsRef<Path>>(&self, path: P) -> Result<Project, AddError> {
        let path = path.as_ref();
        let all_projects = self
            .projects_storage
            .list()
            .context("failed to list projects from storage")?;
        if all_projects.iter().any(|project| project.path == path) {
            return Err(AddError::AlreadyExists);
        }
        if !path.exists() {
            return Err(AddError::PathNotFound);
        }
        if !path.is_dir() {
            return Err(AddError::NotADirectory);
        }
        if !path.join(".git").exists() {
            return Err(AddError::NotAGitRepository);
        };

        if path.join(".gitmodules").exists() {
            return Err(AddError::SubmodulesNotSupported);
        }

        let id = uuid::Uuid::new_v4().to_string();

        // title is the base name of the file
        let title = path
            .iter()
            .last()
            .map_or_else(|| id.clone(), |p| p.to_str().unwrap().to_string());

        let project = Project {
            id: ProjectId::generate(),
            title,
            path: path.to_path_buf(),
            api: None,
            use_diff_context: Some(true),
            ..Default::default()
        };

        // create all required directories to avoid racing later
        let user = self.users.get_user()?;
        let project_repository = project_repository::Repository::open(&project)?;
        gb_repository::Repository::open(&self.local_data_dir, &project_repository, user.as_ref())
            .context("failed to open repository")?;

        self.projects_storage
            .add(&project)
            .context("failed to add project to storage")?;

        // Create a .git/gitbutler directory for app data
        if let Err(error) = std::fs::create_dir_all(project.gb_dir()) {
            tracing::error!(project_id = %project.id, ?error, "failed to create {:?} on project add", project.gb_dir());
        }

        if let Some(watchers) = &self.watchers {
            watchers.watch(&project)?;
        }

        Ok(project)
    }

    pub async fn update(&self, project: &UpdateRequest) -> Result<Project, UpdateError> {
        if let Some(super::AuthKey::Local {
            private_key_path, ..
        }) = &project.preferred_key
        {
            use resolve_path::PathResolveExt;
            let private_key_path = private_key_path.resolve();

            if !private_key_path.exists() {
                return Err(UpdateError::Validation(UpdateValidationError::KeyNotFound(
                    private_key_path.to_path_buf(),
                )));
            }

            if !private_key_path.is_file() {
                return Err(UpdateError::Validation(UpdateValidationError::KeyNotFile(
                    private_key_path.to_path_buf(),
                )));
            }
        }

        let updated = self
            .projects_storage
            .update(project)
            .map_err(|error| match error {
                super::storage::Error::NotFound => UpdateError::NotFound,
                error => UpdateError::Other(error.into()),
            })?;

        if let Some(watchers) = &self.watchers {
            if let Some(api) = &project.api {
                if api.sync {
                    if let Err(error) = watchers
                        .post(watcher::Event::FetchGitbutlerData(project.id))
                        .await
                    {
                        tracing::error!(
                            project_id = %project.id,
                            ?error,
                            "failed to post fetch project event"
                        );
                    }
                }

                if let Err(error) = watchers
                    .post(watcher::Event::PushGitbutlerData(project.id))
                    .await
                {
                    tracing::error!(
                        project_id = %project.id,
                        ?error,
                        "failed to post push project event"
                    );
                }
            }
        }

        Ok(updated)
    }

    pub fn get(&self, id: &ProjectId) -> Result<Project, GetError> {
        let project = self.projects_storage.get(id).map_err(|error| match error {
            super::storage::Error::NotFound => GetError::NotFound,
            error => GetError::Other(error.into()),
        });
        if let Ok(project) = &project {
            if !project.gb_dir().exists() {
                if let Err(error) = std::fs::create_dir_all(project.gb_dir()) {
                    tracing::error!(project_id = %project.id, ?error, "failed to create {:?} on project get", project.gb_dir());
                }
            }
            // Clean up old virtual_branches.toml that was never used
            if project
                .path
                .join(".git")
                .join("virtual_branches.toml")
                .exists()
            {
                if let Err(error) =
                    std::fs::remove_file(project.path.join(".git").join("virtual_branches.toml"))
                {
                    tracing::error!(project_id = %project.id, ?error, "failed to remove old virtual_branches.toml");
                }
            }
        }
        project
    }

    pub fn list(&self) -> Result<Vec<Project>, ListError> {
        self.projects_storage
            .list()
            .map_err(|error| ListError::Other(error.into()))
    }

    pub async fn delete(&self, id: &ProjectId) -> Result<(), DeleteError> {
        let project = match self.projects_storage.get(id) {
            Ok(project) => Ok(project),
            Err(super::storage::Error::NotFound) => return Ok(()),
            Err(error) => Err(DeleteError::Other(error.into())),
        }?;

        if let Some(watchers) = &self.watchers {
            if let Err(error) = watchers.stop(id).await {
                tracing::error!(
                    project_id = %id,
                    ?error,
                    "failed to stop watcher for project",
                );
            }
        }

        self.projects_storage
            .purge(&project.id)
            .map_err(|error| DeleteError::Other(error.into()))?;

        if let Err(error) = std::fs::remove_dir_all(
            self.local_data_dir
                .join("projects")
                .join(project.id.to_string()),
        ) {
            tracing::error!(project_id = %id, ?error, "failed to remove project data",);
        }

        if let Err(error) = std::fs::remove_file(project.path.join(".git/gitbutler.json")) {
            tracing::error!(project_id = %project.id, ?error, "failed to remove .git/gitbutler.json data",);
        }

        let virtual_branches_path = project.path.join(".git/virtual_branches.toml");
        if virtual_branches_path.exists() {
            if let Err(error) = std::fs::remove_file(virtual_branches_path) {
                tracing::error!(project_id = %project.id, ?error, "failed to remove .git/virtual_branches.toml data",);
            }
        }

        Ok(())
    }

    pub fn get_local_config(
        &self,
        id: &ProjectId,
        key: &str,
    ) -> Result<Option<String>, ConfigError> {
        let project = self.projects_storage.get(id).map_err(|error| match error {
            super::storage::Error::NotFound => ConfigError::NotFound,
            error => ConfigError::Other(error.into()),
        })?;

        let repo = project_repository::Repository::open(&project)
            .map_err(|e| ConfigError::Other(e.into()))?;
        repo.config()
            .get_local(key)
            .map_err(|e| ConfigError::Other(e.into()))
    }

    pub fn set_local_config(
        &self,
        id: &ProjectId,
        key: &str,
        value: &str,
    ) -> Result<(), ConfigError> {
        let project = self.projects_storage.get(id).map_err(|error| match error {
            super::storage::Error::NotFound => ConfigError::NotFound,
            error => ConfigError::Other(error.into()),
        })?;

        let repo = project_repository::Repository::open(&project)
            .map_err(|e| ConfigError::Other(e.into()))?;
        repo.config()
            .set_local(key, value)
            .map_err(|e| ConfigError::Other(e.into()))?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("project not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error(transparent)]
    Other(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error("project not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("project not found")]
    NotFound,
    #[error(transparent)]
    Validation(UpdateValidationError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateValidationError {
    #[error("{0} not found")]
    KeyNotFound(PathBuf),
    #[error("{0} is not a file")]
    KeyNotFile(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("not a directory")]
    NotADirectory,
    #[error("not a git repository")]
    NotAGitRepository,
    #[error("path not found")]
    PathNotFound,
    #[error("project already exists")]
    AlreadyExists,
    #[error("submodules not supported")]
    SubmodulesNotSupported,
    #[error(transparent)]
    User(#[from] users::GetError),
    #[error(transparent)]
    OpenProjectRepository(#[from] project_repository::OpenError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
