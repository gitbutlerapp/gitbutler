use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    project_repository,
    projects::{self, ProjectId},
    users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_store: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            project_store: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

impl Handler {
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        let user = self.users.get_user()?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;

        if project_repository.project().is_sync_enabled()
            && project_repository.project().has_code_url()
        {
            project_repository
                .push_to_gitbutler_server(user.as_ref())
                .context("failed to push project to gitbutler")?;
        } else {
            tracing::debug!(
                %project_id,
                "cannot push code to gb",
            );
        }

        Ok(vec![])
    }
}
