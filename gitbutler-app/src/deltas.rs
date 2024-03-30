pub mod commands {
    use std::collections::HashMap;

    use gitbutler_core::deltas::{controller::ListError, Controller, Delta};
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::error::{Code, Error};

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
    ) -> Result<HashMap<String, Vec<Delta>>, Error> {
        let session_id = session_id.parse().map_err(|_| Error::UserError {
            message: "Malformed session id".to_string(),
            code: Code::Validation,
        })?;
        let project_id = project_id.parse().map_err(|_| Error::UserError {
            code: Code::Validation,
            message: "Malformed project id".to_string(),
        })?;
        handle
            .state::<Controller>()
            .list_by_session_id(&project_id, &session_id, &paths)
            .map_err(Into::into)
    }
}
