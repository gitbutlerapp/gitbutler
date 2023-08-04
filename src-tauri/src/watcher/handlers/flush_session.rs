use std::path;

use anyhow::{anyhow, Context, Result};

use crate::{gb_repository, project_repository, projects, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_store: projects::Storage,
    local_data_dir: path::PathBuf,
    user_store: users::Storage,
}

impl Handler {
    pub fn new(
        local_data_dir: &path::Path,
        project_store: &projects::Storage,
        user_store: &users::Storage,
    ) -> Self {
        Self {
            project_store: project_store.clone(),
            local_data_dir: local_data_dir.to_path_buf(),
            user_store: user_store.clone(),
        }
    }

    pub fn handle(
        &self,
        project_id: &str,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get_project(&project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow!("project not found"))?;

        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            project_id,
            self.project_store.clone(),
            self.user_store.clone(),
        )
        .context("failed to open repository")?;

        let session = gb_repo
            .flush_session(&project_repository::Repository::open(&project)?, session)
            .context("failed to flush session")?;

        Ok(vec![events::Event::Session(
            project_id.to_string(),
            session,
        )])
    }
}
