use std::path;
use std::sync::{Arc, Mutex, TryLockError};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, project_repository, projects, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<HandlerInner>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            inner: Arc::new(HandlerInner::try_from(value)?),
        })
    }
}

impl Handler {
    pub fn handle(&self, project_id: &str) -> Result<Vec<events::Event>> {
        self.inner.handle(project_id)
    }
}

struct HandlerInner {
    local_data_dir: path::PathBuf,
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
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("failed to get local data dir")?;
        Ok(Self::new(
            local_data_dir.to_path_buf(),
            projects::Controller::try_from(value)?,
            users::Controller::try_from(value)?,
        ))
    }
}

impl HandlerInner {
    fn new(
        local_data_dir: path::PathBuf,
        project_storage: projects::Controller,
        user_storage: users::Controller,
    ) -> Self {
        Self {
            local_data_dir,
            projects: project_storage,
            users: user_storage,
            mutex: Mutex::new(()),
        }
    }

    pub fn handle(&self, project_id: &str) -> Result<Vec<events::Event>> {
        let _lock = match self.mutex.try_lock() {
            Ok(lock) => lock,
            Err(TryLockError::Poisoned(_)) => return Err(anyhow::anyhow!("mutex poisoned")),
            Err(TryLockError::WouldBlock) => return Ok(vec![]),
        };

        let user = self.users.get_user()?;
        let project = self.projects.get(project_id)?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        gb_repo.push(user.as_ref()).context("failed to push")?;

        Ok(vec![])
    }
}
