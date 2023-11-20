use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::paths::DataDir;
use crate::projects::ProjectId;
use crate::{gb_repository, project_repository, projects, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(
            DataDir::try_from(value)?,
            projects::Controller::try_from(value)?,
            users::Controller::from(value),
        ))
    }
}

impl Handler {
    fn new(
        local_data_dir: DataDir,
        projects: projects::Controller,
        users: users::Controller,
    ) -> Self {
        Self {
            local_data_dir,
            projects,
            users,
        }
    }

    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let user = self.users.get_user()?;
        let project = self.projects.get(project_id)?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        gb_repo.push(user.as_ref()).context("failed to push")?;

        Ok(vec![])
    }
}
