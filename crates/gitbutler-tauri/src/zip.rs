pub mod commands {
    #![allow(clippy::used_underscore_binding)]
    use but_api::commands::zip::{self, GetLogsArchivePathParams, GetProjectArchivePathParams};
    use but_api::error::Error;
    use std::path::PathBuf;
    use tauri::State;
    use tracing::instrument;

    #[tauri::command(async)]
    #[instrument(skip(app), err(Debug))]
    pub fn get_project_archive_path(
        app: State<'_, but_api::App>,
        project_id: &str,
    ) -> Result<PathBuf, Error> {
        zip::get_project_archive_path(
            &app,
            GetProjectArchivePathParams {
                project_id: project_id.to_string(),
            },
        )
    }

    #[tauri::command(async)]
    #[instrument(skip(app), err(Debug))]
    pub fn get_logs_archive_path(app: State<'_, but_api::App>) -> Result<PathBuf, Error> {
        zip::get_logs_archive_path(&app, GetLogsArchivePathParams {})
    }
}
