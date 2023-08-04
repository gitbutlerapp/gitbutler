use std::{path, time};

use anyhow::{Context, Result};

use crate::{gb_repository, keys, project_repository, projects, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_storage: projects::Storage,
    local_data_dir: path::PathBuf,
    user_storage: users::Storage,
    keys_controller: keys::Controller,
}

impl Handler {
    pub fn new(
        project_storage: &projects::Storage,
        local_data_dir: &path::Path,
        user_storage: &users::Storage,
        keys_controller: &keys::Controller,
    ) -> Self {
        Self {
            project_storage: project_storage.clone(),
            local_data_dir: local_data_dir.to_path_buf(),
            user_storage: user_storage.clone(),
            keys_controller: keys_controller.clone(),
        }
    }

    pub fn handle(&self, project_id: &str, now: time::SystemTime) -> Result<Vec<events::Event>> {
        let project = self
            .project_storage
            .get_project(&project_id)
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

        let gb_repo = gb_repository::Repository::open(
            self.local_data_dir.clone(),
            project_id,
            self.project_storage.clone(),
            self.user_storage.clone(),
        )
        .context("failed to open repository")?;

        let default_target = gb_repo.default_target()?.context("target not set")?;
        let key = self.keys_controller.get_or_create()?;

        let fetch_result =
            if let Err(err) = project_repository.fetch(&default_target.remote_name, &key) {
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
                id: project_id.to_string(),
                project_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .context("failed to update project")?;

        Ok(vec![])
    }
}
