use std::{
    sync::{Arc, Mutex, TryLockError},
    time,
};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::paths::DataDir;
use crate::{gb_repository, project_repository, projects, users};

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
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,

    // it's ok to use mutex here, because even though project_id is a paramenter, we create
    // and use a handler per project.
    // if that changes, we'll need to use a more granular locking mechanism
    mutex: Mutex<()>,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let local_data_dir = DataDir::try_from(value)?;
        Ok(Self {
            local_data_dir,
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::try_from(value)?,
            mutex: Mutex::new(()),
        })
    }
}

impl HandlerInner {
    pub fn handle(&self, project_id: &str, now: &time::SystemTime) -> Result<Vec<events::Event>> {
        let _lock = match self.mutex.try_lock() {
            Ok(lock) => lock,
            Err(TryLockError::Poisoned(_)) => return Err(anyhow::anyhow!("mutex poisoned")),
            Err(TryLockError::WouldBlock) => return Ok(vec![]),
        };

        let user = self.users.get_user()?;

        // mark fetching
        self.projects
            .update(&projects::UpdateRequest {
                id: project_id.to_string(),
                gitbutler_data_last_fetched: Some(projects::FetchResult::Fetching {
                    timestamp_ms: now.duration_since(time::UNIX_EPOCH)?.as_millis(),
                }),
                ..Default::default()
            })
            .context("failed to mark project as fetching")?;

        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let sessions_before_fetch = gb_repo
            .get_sessions_iterator()?
            .filter_map(|s| s.ok())
            .collect::<Vec<_>>();

        let fetch_result = if let Err(error) = gb_repo.fetch(user.as_ref()) {
            tracing::error!(project_id, ?error, "failed to fetch gitbutler data");
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
                error: error.to_string(),
            }
        } else {
            projects::FetchResult::Fetched {
                timestamp_ms: now.duration_since(time::UNIX_EPOCH)?.as_millis(),
            }
        };

        self.projects
            .update(&projects::UpdateRequest {
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
