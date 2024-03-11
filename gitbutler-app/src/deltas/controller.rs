use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{projects::ProjectId, sessions::SessionId};

use super::{database, Delta};

#[derive(Clone)]
pub struct Controller {
    database: database::Database,
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

    pub fn list_by_session_id_and_filter<
        P: AsRef<Path> + for<'a> PartialEq<&'a Path>,
        F: AsRef<[P]>,
    >(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        paths: F,
    ) -> Result<HashMap<PathBuf, Vec<Delta>>, ListError> {
        Ok(self
            .database
            .list_by_project_id_session_id_and_filter(project_id, session_id, paths)?)
    }

    pub fn list_by_session_id(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
    ) -> Result<HashMap<PathBuf, Vec<Delta>>, ListError> {
        Ok(self
            .database
            .list_by_project_id_session_id(project_id, session_id)?)
    }
}
