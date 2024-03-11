use std::path::{self, Path};

use crate::projects::{self, ProjectId};

use super::Zipper;

#[derive(Clone)]
pub struct Controller {
    local_data_dir: path::PathBuf,
    logs_dir: path::PathBuf,
    zipper: Zipper,
    projects_controller: projects::Controller,
}

impl Controller {
    pub fn new<Pd: AsRef<Path>, Pl: AsRef<Path>>(
        local_data_dir: Pd,
        logs_dir: Pl,
        zipper: Zipper,
        projects_controller: projects::Controller,
    ) -> Self {
        Self {
            local_data_dir: local_data_dir.as_ref().to_path_buf(),
            logs_dir: logs_dir.as_ref().to_path_buf(),
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
