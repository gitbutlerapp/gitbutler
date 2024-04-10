use std::path::PathBuf;
use std::{
    path,
    sync::{Arc, Mutex, TryLockError},
};

use anyhow::{Context, Result};
use gitbutler_core::{
    gb_repository, gb_repository::RemoteError, project_repository, projects, projects::ProjectId,
    users,
};
use tauri::{AppHandle, Manager};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<HandlerInner>>,
}

impl Handler {
    pub fn from_app(app: &AppHandle, app_data_dir: impl Into<PathBuf>) -> Self {
        let projects = app.state::<projects::Controller>().inner().clone();
        let users = app.state::<users::Controller>().inner().clone();
        let inner = HandlerInner {
            local_data_dir: app_data_dir.into(),
            projects,
            users,
        };
        Handler {
            inner: Arc::new(inner.into()),
        }
    }
}

impl Handler {
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        match self.inner.try_lock() {
            Ok(inner) => inner.handle(project_id),
            Err(TryLockError::Poisoned(_)) => Err(anyhow::anyhow!("mutex poisoned")),
            Err(TryLockError::WouldBlock) => Ok(vec![]),
        }
    }
}

struct HandlerInner {
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    users: users::Controller,
}

impl HandlerInner {
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
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

        match gb_repo.push(user.as_ref()) {
            Ok(()) | Err(RemoteError::Network) => Ok(vec![]),
            Err(err) => Err(err).context("failed to push"),
        }
    }
}
