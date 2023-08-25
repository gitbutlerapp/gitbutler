use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{path, time};

use anyhow::{Context, Result};
use rsevents_extra::Semaphore;
use tauri::AppHandle;

use crate::{gb_repository, projects, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<HandlerInner>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let inner = HandlerInner::try_from(value)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
}

impl Handler {
    pub fn handle(&self, project_id: &str, now: &time::SystemTime) -> Result<Vec<events::Event>> {
        self.inner.handle(project_id, now)
    }
}

struct HandlerInner {
    local_data_dir: path::PathBuf,
    project_storage: projects::Storage,
    user_storage: users::Storage,

    semaphores: Arc<Mutex<HashMap<String, Semaphore>>>,
}

impl TryFrom<&AppHandle> for HandlerInner {
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
            semaphores: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

impl HandlerInner {
    pub fn handle(&self, project_id: &str, now: &time::SystemTime) -> Result<Vec<events::Event>> {
        // one fetch at a time
        let mut semaphores = self.semaphores.lock().unwrap();
        let _guard = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(0, 1))
            .wait();

        let gb_repo = gb_repository::Repository::open(
            self.local_data_dir.clone(),
            project_id,
            self.project_storage.clone(),
            self.user_storage.clone(),
        )
        .context("failed to open repository")?;

        let sessions_before_fetch = gb_repo
            .get_sessions_iterator()?
            .filter_map(|s| s.ok())
            .collect::<Vec<_>>();

        // mark fetching
        self.project_storage
            .update_project(&projects::UpdateRequest {
                id: project_id.to_string(),
                gitbutler_data_last_fetched: Some(projects::FetchResult::Fetching {
                    timestamp_ms: now.duration_since(time::UNIX_EPOCH)?.as_millis(),
                }),
                ..Default::default()
            })
            .context("failed to mark project as fetching")?;

        let project = self
            .project_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;

        let fetch_result = if let Err(err) = gb_repo.fetch() {
            tracing::error!("{}: failed to fetch gitbutler data: {:#}", project_id, err);
            projects::FetchResult::Error {
                attempt: project
                    .gitbutler_data_last_fetched
                    .as_ref()
                    .map_or(0, |r| match r {
                        projects::FetchResult::Error { attempt, .. } => *attempt + 1,
                        projects::FetchResult::Fetched { .. } => 0,
                        projects::FetchResult::Fetching { .. } => 0,
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
                gitbutler_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .context("failed to update fetched result")?;

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
            .map(|session| events::Event::Session(project_id.to_string(), session))
            .collect::<Vec<_>>();

        Ok(events)
    }
}
