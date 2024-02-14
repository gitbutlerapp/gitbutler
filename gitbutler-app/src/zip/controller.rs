use std::path;

use tauri::{AppHandle, Manager};

use crate::projects::{self, ProjectId};

use super::Zipper;

#[derive(Clone)]
pub struct Controller {
    local_data_dir: path::PathBuf,
    logs_dir: path::PathBuf,
    zipper: Zipper,
    projects_controller: projects::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(controller) = value.try_state::<Controller>() {
            Ok(controller.inner().clone())
        } else {
            let local_data_dir = value
                .path_resolver()
                .app_data_dir()
                .ok_or_else(|| anyhow::anyhow!("failed to get local data dir"))?;
            let logs_dir = value
                .path_resolver()
                .app_log_dir()
                .ok_or_else(|| anyhow::anyhow!("failed to get logs dir"))?;
            let zipper = Zipper::try_from(value)?;
            let projects = projects::Controller::try_from(value)?;
            let controller = Controller::new(local_data_dir, logs_dir, zipper, projects);
            value.manage(controller.clone());
            Ok(controller)
        }
    }
}

impl Controller {
    fn new(
        local_data_dir: path::PathBuf,
        logs_dir: path::PathBuf,
        zipper: Zipper,
        projects_controller: projects::Controller,
    ) -> Self {
        Self {
            local_data_dir,
            logs_dir,
            zipper,
            projects_controller,
        }
    }

    pub fn archive(&self, project_id: &ProjectId) -> Result<path::PathBuf, ArchiveError> {
        let project = self.projects_controller.get(project_id)?;
        self.zipper.zip(project.path).map_err(Into::into)
    }

    pub fn data_archive(&self, project_id: &ProjectId) -> Result<path::PathBuf, DataArchiveError> {
        let project = self.projects_controller.get(project_id)?;
        self.zipper
            .zip(
                self.local_data_dir
                    .join("projects")
                    .join(project.id.to_string()),
            )
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
