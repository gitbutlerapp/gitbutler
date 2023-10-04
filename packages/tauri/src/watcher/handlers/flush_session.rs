use std::path;

use anyhow::{anyhow, Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, project_repository, projects, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    project_store: projects::Storage,
    user_store: users::Storage,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("failed to get local data dir")?;
        Ok(Self {
            local_data_dir: local_data_dir.to_path_buf(),
            project_store: projects::Storage::try_from(value)?,
            user_store: users::Storage::try_from(value)?,
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
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow!("project not found"))?;

        let user = self.user_store.get()?;

        let gb_repo =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open repository")?;

        let session = gb_repo
            .flush_session(
                &project_repository::Repository::open(&project)?,
                session,
                user.as_ref(),
            )
            .context("failed to flush session")?;

        Ok(vec![
            events::Event::Session(project_id.to_string(), session),
            events::Event::PushGitbutlerData(project_id.to_string()),
        ])
    }
}
