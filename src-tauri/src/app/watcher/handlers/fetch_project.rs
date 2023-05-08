use std::time;

use anyhow::{Context, Result};

use crate::{app::gb_repository, projects};

use super::events;

pub struct Handler<'handler> {
    project_storage: projects::Storage,
    gb_repository: &'handler gb_repository::Repository,
}

impl<'listener> Handler<'listener> {
    pub fn new(
        project_storage: projects::Storage,
        gb_repository: &'listener gb_repository::Repository,
    ) -> Self {
        Self {
            project_storage,
            gb_repository,
        }
    }

    pub fn handle(&self, project: &projects::Project) -> Result<Vec<events::Event>> {
        if !self.gb_repository.fetch().context("failed to fetch")? {
            return Ok(vec![]);
        }

        self.project_storage
            .update_project(&projects::UpdateRequest {
                id: project.id.clone(),
                last_fetched_ts: Some(
                    time::SystemTime::now()
                        .duration_since(time::UNIX_EPOCH)
                        .context("failed to get time since epoch")?
                        .as_millis()
                        .try_into()
                        .context("failed to convert time to millis")?,
                ),
                ..Default::default()
            })
            .context("failed to update project")?;
        Ok(vec![])
    }
}
