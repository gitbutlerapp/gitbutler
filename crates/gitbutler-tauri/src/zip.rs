pub mod commands {
    #![allow(clippy::used_underscore_binding)]
    use but_api::commands::zip::{self, GetLogsArchivePathParams, GetProjectArchivePathParams};
    use but_api::error::Error;
    use std::path::PathBuf;
    use tauri::State;
    use tracing::instrument;

    #[tauri::command(async)]
    #[instrument(skip(ipc_ctx), err(Debug))]
    pub fn get_project_archive_path(
        ipc_ctx: State<'_, but_api::IpcContext>,
        project_id: &str,
    ) -> Result<PathBuf, Error> {
        zip::get_project_archive_path(
            &ipc_ctx,
            GetProjectArchivePathParams {
                project_id: project_id.to_string(),
            },
        )
    }

    #[tauri::command(async)]
    #[instrument(skip(ipc_ctx), err(Debug))]
    pub fn get_logs_archive_path(
        ipc_ctx: State<'_, but_api::IpcContext>,
    ) -> Result<PathBuf, Error> {
        zip::get_logs_archive_path(&ipc_ctx, GetLogsArchivePathParams {})
    }
}
