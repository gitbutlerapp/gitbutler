use anyhow::Result;

use crate::{
    git::RepositoryExt,
    project_repository,
    projects::{self, ProjectId},
};

#[derive(Clone)]
pub struct Controller {
    projects: projects::Controller,
}

impl Controller {
    pub fn new(projects: projects::Controller) -> Self {
        Self { projects }
    }

    pub async fn remotes(&self, project_id: ProjectId) -> Result<Vec<String>> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::ProjectRepo::open(&project)?;

        project_repository.repo().remotes_as_string()
    }

    pub async fn add_remote(&self, project_id: ProjectId, name: &str, url: &str) -> Result<()> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::ProjectRepo::open(&project)?;

        project_repository.repo().remote(name, url)?;
        Ok(())
    }
}
