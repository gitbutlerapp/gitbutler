use std::sync::Arc;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::{
    events as app_events, project_repository::RemoteError, projects::ProjectId, virtual_branches,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<HandlerInner>>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else {
            let vbranches = virtual_branches::Controller::try_from(value)?;
            let inner = HandlerInner::new(vbranches);
            let handler = Handler::new(inner);
            value.manage(handler.clone());
            Ok(handler)
        }
    }
}

impl Handler {
    fn new(inner: HandlerInner) -> Handler {
        Handler {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

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

impl HandlerInner {
    fn new(vbranches: virtual_branches::Controller) -> HandlerInner {
        HandlerInner { vbranches }
    }

    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        match self.vbranches.fetch_from_target(project_id).await {
            Ok(_)
            | Err(virtual_branches::controller::ControllerError::VerifyError(_))
            | Err(virtual_branches::controller::ControllerError::Action(
                virtual_branches::errors::FetchFromTargetError::DefaultTargetNotSet(_)
                | virtual_branches::errors::FetchFromTargetError::Remote(RemoteError::Network)
                | virtual_branches::errors::FetchFromTargetError::Remote(RemoteError::Auth),
            )) => Ok(vec![events::Event::Emit(app_events::Event::git_fetch(
                project_id,
            ))]),
            Err(error) => Err(error).context("failed to fetch project"),
        }
    }
}
