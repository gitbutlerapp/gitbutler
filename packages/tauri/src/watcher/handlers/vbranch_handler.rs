use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager};

use super::events;
use crate::events as app_events;
use crate::{assets, projects::ProjectId, virtual_branches};

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
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let branches = futures::executor::block_on(async {
            self.vbranch_controller
                .list_virtual_branches(project_id)
                .await
        });

        let branches = match branches {
            Ok(branches) => {
                let branches = futures::executor::block_on(async {
                    self.assets_proxy.proxy_virtual_branches(branches).await
                });
                Ok(branches)
            }
            Err(error) => Err(anyhow!(error)),
        };

        match branches {
            Ok(branches) => Ok(vec![events::Event::Emit(
                app_events::Event::virtual_branches(project_id, &branches.clone()),
            )]),
            Err(error) => Err(error),
        }
    }
}
