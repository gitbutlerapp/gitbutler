use crate::error;
use crate::error::AnyhowContextExt;
use crate::error::Code;
use crate::projects::storage;
use crate::projects::ProjectId;
use anyhow::Result;

use super::snapshot::Oplog;

#[derive(Clone)]
pub struct Controller {
    projects_storage: storage::Storage,
}

impl Controller {
    pub fn new(projects_storage: storage::Storage) -> Self {
        Self { projects_storage }
    }

    pub fn snapshots_enabled(&self, project_id: &ProjectId) -> Result<bool, ConfigError> {
        let project = self
            .projects_storage
            .get(project_id)
            .map_err(|error| match error {
                storage::Error::NotFound => ConfigError::NotFound,
                error => ConfigError::Other(error.into()),
            })?;

        Ok(project.snapshots_enabled())
    }

    pub fn set_snapshots_enabled(
        &self,
        project_id: &ProjectId,
        value: bool,
    ) -> Result<(), ConfigError> {
        let project = self
            .projects_storage
            .get(project_id)
            .map_err(|error| match error {
                storage::Error::NotFound => ConfigError::NotFound,
                error => ConfigError::Other(error.into()),
            })?;

        project.set_snapshots_enabled(value).map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("project not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl error::ErrorWithContext for ConfigError {
    fn context(&self) -> Option<error::Context> {
        match self {
            ConfigError::NotFound => {
                error::Context::new_static(Code::Projects, "Project not found").into()
            }
            ConfigError::Other(error) => error.custom_context_or_root_cause().into(),
        }
    }
}
