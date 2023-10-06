use std::path;

use anyhow::Context;
use tauri::AppHandle;

use crate::projects;

use super::Zipper;

pub struct Controller {
    local_data_dir: path::PathBuf,
    logs_dir: path::PathBuf,
    zipper: Zipper,
    projects_controller: projects::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("failed to get local data dir")?;
        let logs_dir = value
            .path_resolver()
            .app_log_dir()
            .context("failed to get log dir")?;
        Ok(Self {
            local_data_dir,
            logs_dir,
            zipper: Zipper::try_from(value)?,
            projects_controller: projects::Controller::try_from(value)?,
        })
    }
}

impl Controller {
    pub fn archive(&self, project_id: &str) -> Result<path::PathBuf, ArchiveError> {
        let project = self.projects_controller.get_project(project_id)?;
        self.zipper.zip(project.path).map_err(Into::into)
    }

    pub fn data_archive(&self, project_id: &str) -> Result<path::PathBuf, DataArchiveError> {
        let project = self.projects_controller.get_project(project_id)?;
        self.zipper
            .zip(self.local_data_dir.join("projects").join(project.id))
            .map_err(Into::into)
    }

    pub fn logs_archive(&self) -> Result<path::PathBuf, LogsArchiveError> {
        self.zipper.zip(&self.logs_dir).map_err(Into::into)
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
