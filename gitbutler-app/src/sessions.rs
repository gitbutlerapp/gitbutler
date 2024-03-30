pub mod commands {
    use gitbutler_core::sessions::{controller::ListError, Controller, Session};
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::error::{Code, Error};

    impl From<ListError> for Error {
        fn from(value: ListError) -> Self {
            match value {
                ListError::UsersError(error) => Error::from(error),
                ListError::ProjectsError(error) => Error::from(error),
                ListError::ProjectRepositoryError(error) => Error::from(error),
                ListError::Other(error) => {
                    tracing::error!(?error);
                    Error::Unknown
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
    ) -> Result<Vec<Session>, Error> {
        let project_id = project_id.parse().map_err(|_| Error::UserError {
            code: Code::Validation,
            message: "Malformed project id".to_string(),
        })?;
        handle
            .state::<Controller>()
            .list(&project_id, earliest_timestamp_ms)
            .map_err(Into::into)
    }
}
