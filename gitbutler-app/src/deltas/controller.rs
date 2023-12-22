use std::collections::HashMap;

use tauri::AppHandle;

use crate::{projects::ProjectId, sessions::SessionId};

use super::{database, Delta};

pub struct Controller {
    database: database::Database,
}

impl From<&AppHandle> for Controller {
    fn from(handle: &AppHandle) -> Self {
        Self {
            database: database::Database::from(handle),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Controller {
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
