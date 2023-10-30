use std::{
    sync::{Arc, Mutex, TryLockError},
    time,
};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    gb_repository, git, keys,
    paths::DataDir,
    project_repository::{self, RemoteError},
    projects::{self, ProjectId},
    users,
};

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
    pub fn handle(
        &self,
        project_id: &ProjectId,
        now: &time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        self.inner.handle(project_id, now)
    }
}

struct HandlerInner {
    local_data_dir: DataDir,
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
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            keys: keys::Controller::try_from(value)?,
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::try_from(value)?,
            mutex: Mutex::new(()),
        })
    }
}

impl HandlerInner {
    pub fn handle(
        &self,
        project_id: &ProjectId,
        now: &time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        let _lock = match self.mutex.try_lock() {
            Ok(lock) => lock,
            Err(TryLockError::Poisoned(_)) => return Err(anyhow::anyhow!("mutex poisoned")),
            Err(TryLockError::WouldBlock) => return Ok(vec![]),
        };

        let user = self.users.get_user()?;

        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository. Make sure the project is configured correctly.")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let default_target = if let Some(target) = gb_repo.default_target()? {
            target
        } else {
            return Ok(vec![]);
        };

        let credentials = git::credentials::Factory::new(
            &project,
            self.keys.get_or_create().context("failed to get key")?,
            user.as_ref(),
        );

        let policy = backoff::ExponentialBackoffBuilder::new()
            .with_max_elapsed_time(Some(time::Duration::from_secs(10 * 60)))
            .build();

        let fetch_result = match backoff::retry(policy, || {
            project_repository
                .fetch(default_target.branch.remote(), &credentials)
                .map_err(|err| {
                    tracing::warn!(%project_id, ?err, will_retry = true, "failed to fetch project data");
                    backoff::Error::transient(err)
                })
        }) {
            Ok(()) => projects::FetchResult::Fetched { timestamp: *now },
            Err(backoff::Error::Permanent(RemoteError::AuthError)) => {
                projects::FetchResult::Error {
                    timestamp: *now,
                    error: RemoteError::AuthError.to_string(),
                }
            }
            Err(error) => {
                tracing::error!(%project_id, ?error, will_retry = false, "failed to fetch project data");
                projects::FetchResult::Error {
                    timestamp: *now,
                    error: error.to_string(),
                }
            }
        };

        self.projects
            .update(&projects::UpdateRequest {
                id: *project_id,
                project_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .context("failed to update fetch result")?;

        Ok(vec![])
    }
}
