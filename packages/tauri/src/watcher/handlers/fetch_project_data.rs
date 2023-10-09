use std::{
    path,
    sync::{Arc, Mutex, TryLockError},
    time,
};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, keys, project_repository, projects, users};

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
    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,

    // it's ok to use mutex here, because even though project_id is a paramenter, we create
    // and use a handler per project.
    // if that changes, we'll need to use a more granular locking mechanism
    mutex: Mutex<()>,
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
            keys: keys::Controller::try_from(value)?,
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
                project_data_last_fetched: Some(projects::FetchResult::Fetching {
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
            .context("failed to open repository. Make sure the project is configured correctly.")?;
        let gb_repo = gb_repository::Repository::open(
            self.local_data_dir.clone(),
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;
        let default_target = gb_repo.default_target()?.context("target not set")?;

        let key = match &project.preferred_key {
            projects::AuthKey::Generated => {
                let private_key = self.keys.get_or_create()?;
                keys::Key::Generated(Box::new(private_key))
            }
            projects::AuthKey::Local {
                private_key_path,
                passphrase,
            } => keys::Key::Local {
                private_key_path: private_key_path.clone(),
                passphrase: passphrase.clone(),
            },
        };

        let fetch_result =
            if let Err(error) = project_repository.fetch(default_target.branch.remote(), &key) {
                tracing::error!(project_id, ?error, "failed to fetch project data");
                projects::FetchResult::Error {
                    attempt: project
                        .project_data_last_fetched
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
                project_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .context("failed to update fetch result")?;

        Ok(vec![])
    }
}
