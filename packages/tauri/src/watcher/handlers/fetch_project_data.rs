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
    project_storage: projects::Storage,
    user_storage: users::Storage,
    keys_controller: keys::Storage,

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
            keys_controller: keys::Storage::try_from(value)?,
            project_storage: projects::Storage::try_from(value)?,
            user_storage: users::Storage::try_from(value)?,
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

        let user = self.user_storage.get()?;

        // mark fetching
        self.project_storage
            .update_project(&projects::UpdateRequest {
                id: project_id.to_string(),
                project_data_last_fetched: Some(projects::FetchResult::Fetching {
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

        let gb_repo =
            gb_repository::Repository::open(self.local_data_dir.clone(), &project, user.as_ref())
                .context("failed to open repository")?;
        let default_target = gb_repo.default_target()?.context("target not set")?;

        let key = match &project.preferred_key {
            projects::AuthKey::Generated => {
                let private_key = self.keys_controller.get_or_create()?;
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
        let project_repository = project_repository::Repository::open(&project)?;

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

        self.project_storage
            .update_project(&projects::UpdateRequest {
                id: project_id.to_string(),
                project_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .context("failed to update fetch result")?;

        Ok(vec![])
    }
}
