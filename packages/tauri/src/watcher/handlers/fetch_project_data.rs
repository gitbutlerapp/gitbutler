use std::sync::Arc;

use anyhow::{Context, Result};
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::{project_repository::RemoteError, projects::ProjectId, virtual_branches};

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
    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        if let Ok(inner) = self.inner.try_lock() {
            inner.handle(project_id).await
        } else {
            Ok(vec![])
        }
    }
}

struct HandlerInner {
    vbranches: virtual_branches::Controller,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            vbranches: virtual_branches::Controller::try_from(value)?,
        })
    }
}

impl HandlerInner {
    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        match self.vbranches.fetch_from_target(project_id).await {
            Ok(_)
            | Err(virtual_branches::controller::ControllerError::VerifyError(_))
            | Err(virtual_branches::controller::ControllerError::Action(
                virtual_branches::errors::FetchFromTargetError::DefaultTargetNotSet(_)
                | virtual_branches::errors::FetchFromTargetError::Remote(RemoteError::Network)
                | virtual_branches::errors::FetchFromTargetError::Remote(RemoteError::Auth),
            )) => Ok(vec![]),
            Err(error) => Err(error).context("failed to fetch project"),
        }
    }
}
