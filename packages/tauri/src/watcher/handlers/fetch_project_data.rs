use std::sync::Arc;

use anyhow::{Context, Result};
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::{projects::ProjectId, virtual_branches};

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
        self.vbranches
            .fetch_from_target(project_id)
            .await
            .context("fetching from target")?;
        Ok(vec![])
    }
}
