use std::{path, time};

use futures::executor::block_on;
use tauri::{AppHandle, Manager};

use crate::{paths::DataDir, watcher};

use super::{storage, storage::UpdateRequest, Project};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: DataDir,
    projects_storage: storage::Storage,
    watchers: Option<watcher::Watchers>,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            projects_storage: storage::Storage::try_from(value)?,
            watchers: Some(value.state::<watcher::Watchers>().inner().clone()),
        })
    }
}

impl From<&DataDir> for Controller {
    fn from(value: &DataDir) -> Self {
        Self {
            local_data_dir: value.clone(),
            projects_storage: storage::Storage::from(value),
            watchers: None,
        }
    }
}

impl Controller {
    pub fn add(&self, path: &path::Path) -> Result<Project, AddError> {
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
            .map_or_else(|| id.clone(), |p| p.to_str().unwrap().to_string());

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

        if let Some(ref watchers) = self.watchers {
            block_on(watchers.watch(&project))?;
        }

        Ok(project)
    }

    pub fn update(&self, project: &UpdateRequest) -> Result<Project, UpdateError> {
        let updated = self
            .projects_storage
            .update(project)
            .map_err(|error| match error {
                super::storage::Error::NotFound => UpdateError::NotFound,
                error => UpdateError::Other(error.into()),
            })?;

        if let Some(ref watchers) = self.watchers {
            if let Err(error) = block_on(watchers.post(watcher::Event::FetchGitbutlerData(
                project.id.clone(),
                time::SystemTime::now(),
            ))) {
                tracing::error!(
                    project_id = &project.id,
                    ?error,
                    "failed to post fetch project event"
                );
            }

            if project.api.is_some() {
                if let Err(error) =
                    block_on(watchers.post(watcher::Event::PushGitbutlerData(project.id.clone())))
                {
                    tracing::error!(
                        project_id = &project.id,
                        ?error,
                        "failed to post push project event"
                    );
                }
            }
        }

        Ok(updated)
    }

    pub fn get(&self, id: &str) -> Result<Project, GetError> {
        self.projects_storage.get(id).map_err(|error| match error {
            super::storage::Error::NotFound => GetError::NotFound,
            error => GetError::Other(error.into()),
        })
    }

    pub fn list(&self) -> Result<Vec<Project>, ListError> {
        self.projects_storage
            .list()
            .map_err(|error| ListError::Other(error.into()))
    }

    pub fn delete(&self, id: &str) -> Result<(), DeleteError> {
        let project = match self.projects_storage.get(id) {
            Ok(project) => Ok(project),
            Err(super::storage::Error::NotFound) => return Ok(()),
            Err(error) => Err(DeleteError::Other(error.into())),
        }?;

        if let Some(ref watchers) = self.watchers {
            if let Err(error) = block_on(watchers.stop(id)) {
                tracing::error!(
                    project_id = id,
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
                .to_path_buf()
                .join("projects")
                .join(&project.id),
        ) {
            tracing::error!(project_id = id, ?error, "failed to remove project data",);
        }

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
