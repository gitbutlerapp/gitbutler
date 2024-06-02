use anyhow::Result;
use std::path;

use super::Zipper;
use crate::projects::{self, ProjectId};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: path::PathBuf,
    logs_dir: path::PathBuf,
    zipper: Zipper,
    #[allow(clippy::struct_field_names)]
    projects_controller: projects::Controller,
}

impl Controller {
    pub fn new(
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

    pub fn archive(&self, project_id: ProjectId) -> Result<path::PathBuf> {
        let project = self.projects_controller.get(project_id)?;
        self.zipper.zip(project.path).map_err(Into::into)
    }

    pub fn data_archive(&self, project_id: ProjectId) -> Result<path::PathBuf> {
        let project = self.projects_controller.get(project_id)?;
        self.zipper
            .zip(
                self.local_data_dir
                    .join("projects")
                    .join(project.id.to_string()),
            )
            .map_err(Into::into)
    }

    pub fn logs_archive(&self) -> Result<path::PathBuf> {
        self.zipper.zip(&self.logs_dir).map_err(Into::into)
    }
}
