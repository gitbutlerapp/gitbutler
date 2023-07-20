use std::time;

use anyhow::{Context, Result};

use crate::{project_repository, projects};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_id: String,
    project_storage: projects::Storage,
}

impl Handler {
    pub fn new(project_id: &str, project_storage: &projects::Storage) -> Self {
        Self {
            project_id: project_id.to_string(),
            project_storage: project_storage.clone(),
        }
    }

    pub fn handle(&self, now: time::SystemTime) -> Result<Vec<events::Event>> {
        let project = self
            .project_storage
            .get_project(&self.project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;

        if !project
            .project_data_last_fetched
            .as_ref()
            .map_or(Ok(true), |r| r.should_fetch(&now))?
        {
            return Ok(vec![]);
        }

        let project_repository = project_repository::Repository::open(&project)?;

        let fetch_result = if let Err(err) = project_repository.fetch() {
            projects::FetchResult::Error {
                attempt: project
                    .project_data_last_fetched
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
                project_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .context("failed to update project")?;

        Ok(vec![])
    }
}
