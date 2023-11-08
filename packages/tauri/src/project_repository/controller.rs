use tauri::{AppHandle, Manager};

use crate::{paths::DataDir, projects::ProjectId, watcher};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: DataDir,
    watchers: Option<watcher::Watchers>,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            watchers: Some(value.state::<watcher::Watchers>().inner().clone()),
        })
    }
}

impl Controller {
    pub fn flush(&self, project_id: &ProjectId) -> Result<(), FlushError> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FlushError {
    #[error(transparent)]
    Other(anyhow::Error),
}
