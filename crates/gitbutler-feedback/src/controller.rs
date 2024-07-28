use std::path::PathBuf;

use anyhow::Result;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;

use crate::zipper::Zipper;

pub struct Archival {
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub projects_controller: projects::Controller,
}

impl Archival {
    fn zipper(&self) -> Zipper {
        Zipper::new(self.cache_dir.clone())
    }
}

impl Archival {
    pub fn archive(&self, project_id: ProjectId) -> Result<PathBuf> {
        let project = self.projects_controller.get(project_id)?;
        self.zipper().zip(project.path).map_err(Into::into)
    }

    pub fn data_archive(&self, project_id: ProjectId) -> Result<PathBuf> {
        let dir_to_archive = self.projects_controller.project_metadata_dir(project_id);
        self.zipper().zip(dir_to_archive).map_err(Into::into)
    }

    pub fn logs_archive(&self) -> Result<PathBuf> {
        self.zipper().zip(&self.logs_dir).map_err(Into::into)
    }
}
