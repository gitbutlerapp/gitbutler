use std::path;

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, projects, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    project_storage: projects::Storage,
    user_storage: users::Storage,
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
            project_storage: projects::Storage::try_from(value)?,
            user_storage: users::Storage::try_from(value)?,
        })
    }
}

impl Handler {
    pub fn handle(&self, project_id: &str) -> Result<Vec<events::Event>> {
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            project_id,
            self.project_storage.clone(),
            self.user_storage.clone(),
        )
        .context("failed to open repository")?;

        gb_repo.push().context("failed to push")?;

        Ok(vec![])
    }
}
