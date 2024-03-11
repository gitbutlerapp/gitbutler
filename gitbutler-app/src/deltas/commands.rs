use std::{collections::HashMap, path::PathBuf};

use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::error::{Code, Error};

use super::{controller::ListError, Controller, Delta};

impl From<ListError> for Error {
    fn from(value: ListError) -> Self {
        match value {
            ListError::Other(error) => {
                tracing::error!(?error);
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_deltas(
    handle: AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<&str>>,
) -> Result<HashMap<PathBuf, Vec<Delta>>, Error> {
    let session_id = session_id.parse().map_err(|_| Error::UserError {
        message: "Malformed session id".to_string(),
        code: Code::Validation,
    })?;
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;

    match paths {
        Some(filters) => Ok(handle.state::<Controller>().list_by_session_id_and_filter(
            &project_id,
            &session_id,
            filters.into_iter().map(PathBuf::from).collect::<Vec<_>>(),
        )?),
        None => Ok(handle
            .state::<Controller>()
            .list_by_session_id(&project_id, &session_id)?),
    }
}
