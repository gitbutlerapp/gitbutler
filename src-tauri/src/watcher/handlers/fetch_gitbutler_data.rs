use std::{path, time};

use anyhow::{Context, Result};

use crate::{gb_repository, projects, users};

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

    pub fn handle(&self, now: time::SystemTime) -> Result<Vec<events::Event>> {
        let project = self
            .project_storage
            .get_project(&self.project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;

        if !project
            .gitbutler_data_last_fetched
            .as_ref()
            .map_or(Ok(true), |r| r.should_fetch(&now))?
        {
            return Ok(vec![]);
        }

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

        let fetch_result = if let Err(err) = gb_repo.fetch() {
            projects::FetchResult::Error {
                attempt: project
                    .gitbutler_data_last_fetched
                    .as_ref()
                    .map_or(0, |r| match r {
                        projects::FetchResult::Error { attempt, .. } => *attempt + 1,
                        projects::FetchResult::Fetched { .. } => 0,
                    }),
                timestamp_ms: now.duration_since(time::UNIX_EPOCH)?.as_millis(),
                error: err.to_string(),
            }
        } else {
            projects::FetchResult::Fetched {
                timestamp_ms: now.duration_since(time::UNIX_EPOCH)?.as_millis(),
            }
        };

        self.project_storage
            .update_project(&projects::UpdateRequest {
                id: self.project_id.clone(),
                gitbutler_data_last_fetched: Some(fetch_result),
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
