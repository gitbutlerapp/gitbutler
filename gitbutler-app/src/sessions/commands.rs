use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::error::{Code, UserError};

use super::{
    controller::{Controller, ListError},
    Session,
};

impl From<ListError> for UserError {
    fn from(value: ListError) -> Self {
        match value {
            ListError::UsersError(error) => UserError::from(error),
            ListError::ProjectsError(error) => UserError::from(error),
            ListError::ProjectRepositoryError(error) => UserError::from(error),
            ListError::Other(error) => {
                tracing::error!(?error);
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_sessions(
    handle: AppHandle,
    project_id: &str,
    earliest_timestamp_ms: Option<u128>,
) -> Result<Vec<Session>, UserError> {
    let project_id = project_id.parse().map_err(|_| UserError::User {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .list(&project_id, earliest_timestamp_ms)
        .map_err(Into::into)
}
