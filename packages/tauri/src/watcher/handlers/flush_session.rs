use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, paths::DataDir, project_repository, projects, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: DataDir,
    project_store: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            project_store: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

impl Handler {
    pub fn handle(
        &self,
        project_id: &str,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        let user = self.users.get_user()?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let session = gb_repo
            .flush_session(&project_repository, session, user.as_ref())
            .context("failed to flush session")?;

        Ok(vec![
            events::Event::Session(project_id.to_string(), session),
            events::Event::PushGitbutlerData(project_id.to_string()),
        ])
    }
}
