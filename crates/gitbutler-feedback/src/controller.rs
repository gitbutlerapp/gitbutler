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
        let project = gitbutler_project::get(project_id)?;
        self.zipper().zip(project.path)
    }

    pub fn logs_archive(&self) -> Result<PathBuf> {
        self.zipper().zip(&self.logs_dir)
    }
}
