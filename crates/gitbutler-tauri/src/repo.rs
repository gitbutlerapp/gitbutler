pub mod commands {
    use crate::error::Error;
    use gitbutler_core::projects::{controller::Controller, ProjectId};
    use tauri::Manager;
    use tracing::instrument;

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn git_get_local_config(
        handle: tauri::AppHandle,
        id: ProjectId,
        key: &str,
    ) -> Result<Option<String>, Error> {
        Ok(handle.state::<Controller>().get_local_config(id, key)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn git_set_local_config(
        handle: tauri::AppHandle,
        id: ProjectId,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        handle
            .state::<Controller>()
            .set_local_config(id, key, value)
            .map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn check_signing_settings(
        handle: tauri::AppHandle,
        id: ProjectId,
    ) -> Result<bool, Error> {
        handle
            .state::<Controller>()
            .check_signing_settings(id)
            .map_err(Into::into)
    }
}
