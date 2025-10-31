pub mod commands {
    #![allow(clippy::used_underscore_binding)]
    use std::path::PathBuf;

    use but_api::{
        commands::zip::{
            self, GetAnonymousGraphPathParams, GetLogsArchivePathParams,
            GetProjectArchivePathParams,
        },
        error::Error,
    };
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
    pub fn get_anonymous_graph_path(
        app: State<'_, but_api::App>,
        project_id: &str,
    ) -> Result<PathBuf, Error> {
        zip::get_anonymous_graph_path(
            &app,
            GetAnonymousGraphPathParams {
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
