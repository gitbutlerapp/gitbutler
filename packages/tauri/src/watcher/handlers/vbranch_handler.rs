use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{assets, events as app_events, projects::ProjectId, virtual_branches};

use super::events;

#[derive(Clone)]
pub struct Handler {
    vbranch_controller: virtual_branches::Controller,
    assets_proxy: assets::Proxy,
}

impl TryFrom<&AppHandle> for Handler {
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

impl Handler {
    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let branches = self
            .vbranch_controller
            .list_virtual_branches(project_id)
            .await
            .context("failed to list virtual branches")?;

        let branches = self.assets_proxy.proxy_virtual_branches(branches).await;

        Ok(vec![events::Event::Emit(
            app_events::Event::virtual_branches(project_id, &branches.clone()),
        )])
    }
}
