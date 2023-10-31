use std::sync::{Arc, Mutex, TryLockError};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    project_repository,
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
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        match self.inner.try_lock() {
            Ok(inner) => inner.handle(project_id),
            Err(TryLockError::Poisoned(_)) => Err(anyhow::anyhow!("mutex poisoned")),
            Err(TryLockError::WouldBlock) => Ok(vec![]),
        }
    }
}

pub struct HandlerInner {
    project_store: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            project_store: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

impl HandlerInner {
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        let user = self.users.get_user()?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;

        if project_repository.project().is_sync_enabled()
            && project_repository.project().has_code_url()
        {
            project_repository
                .push_to_gitbutler_server(user.as_ref())
                .context("failed to push project to gitbutler")
                .expect("");
        } else {
            tracing::debug!(
                %project_id,
                "cannot push code to gb",
            );
        }

        Ok(vec![])
    }
}
