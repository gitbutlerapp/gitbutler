use std::{path, time};

use anyhow::{Context, Result};

use crate::{gb_repository, project_repository, projects, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_id: String,
    project_storage: projects::Storage,
    local_data_dir: path::PathBuf,
    user_storage: users::Storage,
}

impl Handler {
    pub fn new(
        local_data_dir: path::PathBuf,
        project_id: String,
        project_storage: projects::Storage,
        user_storage: users::Storage,
    ) -> Self {
        Self {
            project_id,
            project_storage,
            user_storage,
            local_data_dir,
        }
    }

    pub fn handle(&self) -> Result<Vec<events::Event>> {
        let gb_repo = gb_repository::Repository::open(
            self.local_data_dir.clone(),
            self.project_id.clone(),
            self.project_storage.clone(),
            self.user_storage.clone(),
        )
        .context("failed to open repository")?;

        let sessions_before_fetch = gb_repo
            .get_sessions_iterator()?
            .filter_map(|s| s.ok())
            .collect::<Vec<_>>();

        let mut fetched = false;
        if let Err(err) = gb_repo.fetch() {
            log::error!("failed to fetch: {}", err);
        } else {
            fetched = true
        };

        let project = self
            .project_storage
            .get_project(&self.project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;

        if let Err(err) = project_repository.fetch() {
            log::error!("failed to fetch: {}", err);
        } else {
            fetched = true
        };

        if !fetched {
            return Ok(vec![]);
        }

        self.project_storage
            .update_project(&projects::UpdateRequest {
                id: self.project_id.clone(),
                last_fetched_ts: Some(
                    time::SystemTime::now()
                        .duration_since(time::UNIX_EPOCH)
                        .context("failed to get time since epoch")?
                        .as_millis(),
                ),
                ..Default::default()
            })
            .context("failed to update project")?;

        let sessions_after_fetch = gb_repo
            .get_sessions_iterator()?
            .filter_map(|s| s.ok())
            .collect::<Vec<_>>();

        let new_sessions = sessions_after_fetch
            .iter()
            .filter(|s| !sessions_before_fetch.contains(s))
            .collect::<Vec<_>>();

        let events = new_sessions
            .into_iter()
            .cloned()
            .map(events::Event::Session)
            .collect::<Vec<_>>();

        Ok(events)
    }
}
