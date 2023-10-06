use std::{path, time};

use anyhow::Context;
use tauri::{AppHandle, Manager};

use crate::{gb_repository, users, watcher};

use super::{storage, Project, UpdateRequest};

pub struct Controller {
    local_data_dir: path::PathBuf,
    watchers: watcher::Watchers,
    projects_storage: storage::Storage,
    users_storage: users::Storage,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: value
                .path_resolver()
                .app_local_data_dir()
                .context("failed to get local data dir")?,
            projects_storage: storage::Storage::try_from(value)?,
            users_storage: users::Storage::try_from(value)?,
            watchers: value.state::<watcher::Watchers>().inner().clone(),
        })
    }
}

impl Controller {
    pub async fn add_project(&self, path: &path::Path) -> Result<Project, AddError> {
        let all_projects = self
            .projects_storage
            .list()
            .map_err(|error| AddError::Other(error.into()))?;
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

        let id = uuid::Uuid::new_v4().to_string();

        // title is the base name of the file
        let title = path
            .iter()
            .last()
            .map(|p| p.to_str().unwrap().to_string())
            .unwrap_or_else(|| id.clone());

        let project = Project {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            path: path.to_path_buf(),
            api: None,
            ..Default::default()
        };

        self.projects_storage
            .add(&project)
            .map_err(|error| AddError::Other(error.into()))?;

        self.watchers.watch(&project).await?;

        Ok(project)
    }

    pub async fn update_project(&self, project: &UpdateRequest) -> Result<Project, UpdateError> {
        let updated = self
            .projects_storage
            .update(project)
            .map_err(|error| match error {
                super::StorageError::NotFound => UpdateError::NotFound,
                error => UpdateError::Other(error.into()),
            })?;

        if let Err(error) = self
            .watchers
            .post(watcher::Event::FetchGitbutlerData(
                project.id.clone(),
                time::SystemTime::now(),
            ))
            .await
        {
            tracing::error!(
                project_id = &project.id,
                ?error,
                "failed to post fetch project event"
            );
        }

        if project.api.is_some() {
            if let Err(error) = self
                .watchers
                .post(watcher::Event::PushGitbutlerData(project.id.clone()))
                .await
            {
                tracing::error!(
                    project_id = &project.id,
                    ?error,
                    "failed to post push project event"
                );
            }
        }

        Ok(updated)
    }

    pub fn get_project(&self, id: &str) -> Result<Project, GetError> {
        self.projects_storage.get(id).map_err(|error| match error {
            super::StorageError::NotFound => GetError::NotFound,
            error => GetError::Other(error.into()),
        })
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, ListError> {
        self.projects_storage
            .list()
            .map_err(|error| ListError::Other(error.into()))
    }

    pub async fn delete_project(&self, id: &str) -> Result<(), DeleteError> {
        let project = match self.projects_storage.get(id) {
            Ok(project) => Ok(project),
            Err(super::StorageError::NotFound) => return Ok(()),
            Err(error) => Err(DeleteError::Other(error.into())),
        }?;

        if let Err(error) = self.watchers.stop(id).await {
            tracing::error!(
                project_id = id,
                ?error,
                "failed to stop watcher for project",
            );
        }

        match gb_repository::Repository::open(
            self.local_data_dir.clone(),
            &project,
            self.users_storage
                .get()
                .map_err(|error| DeleteError::Other(error.into()))?
                .as_ref(),
        ) {
            Ok(gb_repository) => {
                if let Err(error) = gb_repository.purge() {
                    tracing::error!(
                        project_id = project.id,
                        ?error,
                        "failed to remove project dir"
                    );
                }
                Ok(())
            }
            Err(gb_repository::Error::ProjectPathNotFound(_)) => Ok(()),
            Err(error) => Err(DeleteError::Other(error.into())),
        }?;

        self.projects_storage
            .purge(&project.id)
            .map_err(|error| DeleteError::Other(error.into()))?;

        Ok(())
    }
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
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("not a directory")]
    NotADirectory,
    #[error("not a git repository")]
    NotAGitRepository,
    #[error("failed to create project")]
    PathNotFound,
    #[error("project already exists")]
    AlreadyExists,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
