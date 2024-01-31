use std::path;

use tauri::AppHandle;

use crate::{
    paths::LogsDir,
    projects::{self, ProjectId},
};

use super::Zipper;

pub struct Controller {
    local_data_dir: path::PathBuf,
    logs_dir: LogsDir,
    zipper: Zipper,
    projects_controller: projects::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let local_data_dir = value.path_resolver().app_data_dir();
        match local_data_dir {
            Some(local_data_dir) => {
                let logs_dir = LogsDir::try_from(value)?;
                Ok(Self {
                    local_data_dir,
                    logs_dir,
                    zipper: Zipper::try_from(value)?,
                    projects_controller: projects::Controller::try_from(value)?,
                })
            }
            None => Err(anyhow::anyhow!("failed to get app data dir")),
        }
    }
}

impl Controller {
    pub fn archive(&self, project_id: &ProjectId) -> Result<path::PathBuf, ArchiveError> {
        let project = self.projects_controller.get(project_id)?;
        self.zipper.zip(project.path).map_err(Into::into)
    }

    pub fn data_archive(&self, project_id: &ProjectId) -> Result<path::PathBuf, DataArchiveError> {
        let project = self.projects_controller.get(project_id)?;
        self.zipper
            .zip(
                self.local_data_dir
                    .to_path_buf()
                    .join("projects")
                    .join(project.id.to_string()),
            )
            .map_err(Into::into)
    }

    pub fn logs_archive(&self) -> Result<path::PathBuf, LogsArchiveError> {
        self.zipper
            .zip(self.logs_dir.to_path_buf())
            .map_err(Into::into)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ArchiveError {
    #[error(transparent)]
    GetProject(#[from] projects::GetError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum DataArchiveError {
    #[error(transparent)]
    GetProject(#[from] projects::GetError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum LogsArchiveError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
