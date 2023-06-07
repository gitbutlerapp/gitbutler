use std::time;

use anyhow::{Context, Result};

use crate::projects;

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_id: String,
    project_storage: projects::Storage,
}

impl Handler {
    pub fn new(project_id: String, project_storage: projects::Storage) -> Self {
        Self {
            project_id,
            project_storage,
        }
    }

    pub fn handle(&self, now: time::SystemTime) -> Result<Vec<events::Event>> {
        match self
            .project_storage
            .get_project(&self.project_id)
            .context("failed to get project")?
        {
            None => Ok(vec![]),
            Some(project) => {
                if should_fetch(now, &project)? {
                    Ok(vec![events::Event::Fetch])
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

const TEN_MINUTES: time::Duration = time::Duration::new(10 * 60, 0);

pub(super) fn should_fetch(now: time::SystemTime, project: &projects::Project) -> Result<bool> {
    if project.last_fetched_ts.is_none() {
        return Ok(true);
    }
    let project_last_fetch = time::UNIX_EPOCH
        + time::Duration::from_millis(project.last_fetched_ts.unwrap().try_into()?);
    Ok(project_last_fetch + TEN_MINUTES < now)
}
