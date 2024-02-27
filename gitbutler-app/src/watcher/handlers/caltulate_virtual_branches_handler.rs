use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use governor::{
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
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
    inner: Arc<Mutex<InnerHandler>>,
    limit: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else {
            let vbranches = virtual_branches::Controller::try_from(value)?;
            let proxy = assets::Proxy::try_from(value)?;
            let inner = InnerHandler::new(vbranches, proxy);
            let handler = Handler::new(inner);
            value.manage(handler.clone());
            Ok(handler)
        }
    }
}

impl Handler {
    fn new(inner: InnerHandler) -> Self {
        let quota = Quota::with_period(Duration::from_millis(100)).expect("valid quota");
        Self {
            inner: Arc::new(Mutex::new(inner)),
            limit: Arc::new(RateLimiter::direct(quota)),
        }
    }

    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        if self.limit.check().is_err() {
            Ok(vec![])
        } else if let Ok(handler) = self.inner.try_lock() {
            handler.handle(project_id).await
        } else {
            Ok(vec![])
        }
    }
}

struct InnerHandler {
    vbranch_controller: virtual_branches::Controller,
    assets_proxy: assets::Proxy,
}

impl InnerHandler {
    fn new(vbranch_controller: virtual_branches::Controller, assets_proxy: assets::Proxy) -> Self {
        Self {
            vbranch_controller,
            assets_proxy,
        }
    }

    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        match self
            .vbranch_controller
            .list_virtual_branches(project_id)
            .await
        {
            Ok((branches, _)) => Ok(vec![events::Event::Emit(
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
