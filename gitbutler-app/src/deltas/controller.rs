use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::{projects::ProjectId, sessions::SessionId};

use super::{database, Delta};

#[derive(Clone)]
pub struct Controller {
    database: database::Database,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(value.state::<Self>().inner().clone())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Controller {
    pub fn new(database: database::Database) -> Controller {
        Controller { database }
    }

    pub fn list_by_session_id(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        paths: &Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<Delta>>, ListError> {
        self.database
            .list_by_project_id_session_id(project_id, session_id, paths)
            .map_err(Into::into)
    }
}
