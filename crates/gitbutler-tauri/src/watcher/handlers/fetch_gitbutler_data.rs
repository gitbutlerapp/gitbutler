use std::{path, sync::Arc, time};

use anyhow::{Context, Result};
use gitbutler_core::{gb_repository, project_repository, projects, projects::ProjectId, users};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use super::events;

#[derive(Clone)]
pub struct Handler {
    state: Arc<Mutex<State>>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let projects = value.state::<projects::Controller>().inner().clone();
            let users = value.state::<users::Controller>().inner().clone();
            let handler = Handler::new(app_data_dir, projects, users);
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

impl Handler {
    pub fn new(
        local_data_dir: path::PathBuf,
        projects: projects::Controller,
        users: users::Controller,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                local_data_dir,
                projects,
                users,
            })),
        }
    }

    pub async fn handle(
        &self,
        project_id: &ProjectId,
        now: &time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        if let Ok(state) = self.state.try_lock() {
            Self::handle_inner(&state, project_id, now).await
        } else {
            Ok(vec![])
        }
    }

    async fn handle_inner(
        state: &State,
        project_id: &ProjectId,
        now: &time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        let user = state.users.get_user()?;

        let project = state
            .projects
            .get(project_id)
            .context("failed to get project")?;

        if !project.api.as_ref().map(|api| api.sync).unwrap_or_default() {
            anyhow::bail!("sync disabled");
        }

        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &state.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let sessions_before_fetch = gb_repo
            .get_sessions_iterator()?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let policy = backoff::ExponentialBackoffBuilder::new()
            .with_max_elapsed_time(Some(time::Duration::from_secs(10 * 60)))
            .build();

        let fetch_result = match backoff::retry(policy, || {
            gb_repo.fetch(user.as_ref()).map_err(|err| {
                match err  {
                    gb_repository::RemoteError::Network => backoff::Error::permanent(err),
                    err @ gb_repository::RemoteError::Other(_) =>  {
                        tracing::warn!(%project_id, ?err, will_retry = true, "failed to fetch project data");
                        backoff::Error::transient(err)
                    }
                }
            })
        }) {
            Ok(()) => projects::FetchResult::Fetched { timestamp: *now },
            Err(backoff::Error::Permanent(gb_repository::RemoteError::Network)) => {
                projects::FetchResult::Error {
                    timestamp: *now,
                    error: "network error".to_string(),
                }
            }
            Err(error) => {
                tracing::error!(%project_id, ?error, will_retry=false, "failed to fetch gitbutler data");
                projects::FetchResult::Error {
                    timestamp: *now,
                    error: error.to_string(),
                }
            }
        };

        state
            .projects
            .update(&projects::UpdateRequest {
                id: *project_id,
                gitbutler_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .await
            .context("failed to update fetched result")?;

        let sessions_after_fetch = gb_repo
            .get_sessions_iterator()?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let new_sessions = sessions_after_fetch
            .iter()
            .filter(|s| !sessions_before_fetch.contains(s))
            .collect::<Vec<_>>();

        let events = new_sessions
            .into_iter()
            .cloned()
            .map(|session| events::Event::Session(*project_id, session))
            .collect::<Vec<_>>();

        Ok(events)
    }
}

struct State {
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    users: users::Controller,
}
