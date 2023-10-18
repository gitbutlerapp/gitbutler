use std::{
    sync::{Arc, Mutex, TryLockError},
    time,
};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, project_repository, projects, users};
use crate::{paths::DataDir, projects::ProjectId};

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

        if !project.api.as_ref().map(|api| api.sync).unwrap_or_default() {
            //TODO: make the whole handler use a typesafe error
            anyhow::bail!("sync disabled");
        }

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
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let policy = backoff::ExponentialBackoffBuilder::new()
            .with_max_elapsed_time(Some(time::Duration::from_secs(10 * 60)))
            .build();

        let fetch_result = if let Err(error) = backoff::retry(policy, || {
            gb_repo.fetch(user.as_ref()).map_err(|err| {
                tracing::warn!(%project_id, ?err, will_retry=true, "failed to fetch gitbutler data" );
                backoff::Error::transient(err)
            })
        }) {
            tracing::error!(%project_id, ?error, will_retry=false, "failed to fetch gitbutler data");
            projects::FetchResult::Error {
                timestamp: *now,
                error: error.to_string(),
            }
        } else {
            projects::FetchResult::Fetched { timestamp: *now }
        };

        self.projects
            .update(&projects::UpdateRequest {
                id: *project_id,
                gitbutler_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
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

#[cfg(test)]
mod test {
    use std::time::SystemTime;

    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use crate::test_utils::{Case, Suite};

    use super::*;

    fn remote_repository() -> Result<git2::Repository> {
        let path = tempdir()?.path().to_str().unwrap().to_string();
        let repository = git2::Repository::init_bare(path)?;
        Ok(repository)
    }

    #[test]
    fn test_fetch_success() -> Result<()> {
        let suite = Suite::default();
        let Case { project, .. } = suite.new_case();

        let cloud = remote_repository()?;

        let api_project = projects::ApiProject {
            name: "test-sync".to_string(),
            description: None,
            repository_id: "123".to_string(),
            git_url: cloud.path().to_str().unwrap().to_string(),
            created_at: 0_i32.to_string(),
            updated_at: 0_i32.to_string(),
            sync: true,
        };

        suite.projects.update(&projects::UpdateRequest {
            id: project.id,
            api: Some(api_project.clone()),
            ..Default::default()
        })?;

        let listener = HandlerInner {
            local_data_dir: suite.local_app_data,
            projects: suite.projects,
            users: suite.users,
            mutex: Mutex::new(()),
        };

        listener.handle(&project.id, &SystemTime::now()).unwrap();

        Ok(())
    }

    #[test]
    fn test_fetch_fail_no_sync() {
        let suite = Suite::default();
        let Case { project, .. } = suite.new_case();

        let listener = HandlerInner {
            local_data_dir: suite.local_app_data,
            projects: suite.projects,
            users: suite.users,
            mutex: Mutex::new(()),
        };

        let res = listener.handle(&project.id, &SystemTime::now());

        assert_eq!(&res.unwrap_err().to_string(), "sync disabled");
    }
}
