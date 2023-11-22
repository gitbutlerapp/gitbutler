use std::{sync::Arc, time};

use anyhow::{Context, Result};
use tauri::AppHandle;
use tokio::sync::Mutex;

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
    inner: Arc<Mutex<HandlerInner>>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let inner = HandlerInner::try_from(value)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

impl Handler {
    pub async fn handle(
        &self,
        project_id: &ProjectId,
        now: &time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        if let Ok(inner) = self.inner.try_lock() {
            inner.handle(project_id, now).await
        } else {
            Ok(vec![])
        }
    }
}

struct HandlerInner {
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            keys: keys::Controller::from(value),
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

impl HandlerInner {
    pub async fn handle(
        &self,
        project_id: &ProjectId,
        now: &time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        let user = self.users.get_user()?;

        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        let project_repository = project_repository::Repository::open(&project)
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
            Err(backoff::Error::Permanent(RemoteError::Network)) => projects::FetchResult::Error {
                timestamp: *now,
                error: RemoteError::Network.to_string(),
            },
            Err(backoff::Error::Permanent(RemoteError::Auth)) => projects::FetchResult::Error {
                timestamp: *now,
                error: RemoteError::Auth.to_string(),
            },
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
            .await
            .context("failed to update fetch result")?;

        Ok(vec![])
    }
}
