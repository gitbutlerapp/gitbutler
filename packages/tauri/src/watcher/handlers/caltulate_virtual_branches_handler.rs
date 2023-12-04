use std::sync::Arc;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::{
    assets, events as app_events,
    projects::ProjectId,
    virtual_branches::{self, controller::ControllerError},
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
    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        if let Ok(handler) = self.inner.try_lock() {
            handler.handle(project_id).await
        } else {
            Ok(vec![])
        }
    }
}

struct HandlerInner {
    vbranch_controller: virtual_branches::Controller,
    assets_proxy: assets::Proxy,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;
    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            vbranch_controller: value
                .state::<virtual_branches::Controller>()
                .inner()
                .clone(),
            assets_proxy: value.state::<assets::Proxy>().inner().clone(),
        })
    }
}

impl HandlerInner {
    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        match self
            .vbranch_controller
            .list_virtual_branches(project_id)
            .await
        {
            Ok(branches) => Ok(vec![events::Event::Emit(
                app_events::Event::virtual_branches(
                    project_id,
                    &self.assets_proxy.proxy_virtual_branches(branches).await,
                ),
            )]),
            Err(ControllerError::VerifyError(_)) => Ok(vec![]),
            Err(error) => Err(error).context("failed to list virtual branches"),
        }
    }
}
