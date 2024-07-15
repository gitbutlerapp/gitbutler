pub mod commands {
    #![allow(clippy::used_underscore_binding)]
    use anyhow::Context;
    use gitbutler_error::error;
    use gitbutler_error::error::Code;
    use gitbutler_feedback::Archival;
    use std::path::PathBuf;
    use tauri::State;
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(archival), err(Debug))]
    pub async fn get_project_archive_path(
        archival: State<'_, Archival>,
        project_id: &str,
    ) -> Result<PathBuf, Error> {
        let project_id = project_id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        archival.archive(project_id).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(archival), err(Debug))]
    pub async fn get_project_data_archive_path(
        archival: State<'_, Archival>,
        project_id: &str,
    ) -> Result<PathBuf, Error> {
        let project_id = project_id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        archival.data_archive(project_id).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(archival), err(Debug))]
    pub async fn get_logs_archive_path(archival: State<'_, Archival>) -> Result<PathBuf, Error> {
        archival.logs_archive().map_err(Into::into)
    }
}
