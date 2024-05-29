pub mod commands {
    #![allow(clippy::used_underscore_binding)]
    use anyhow::Context;
    use std::path;

    use gitbutler_core::error;
    use gitbutler_core::error::Code;
    use gitbutler_core::zip::controller;
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_project_archive_path(
        handle: AppHandle,
        project_id: &str,
    ) -> Result<path::PathBuf, Error> {
        let project_id = project_id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        handle
            .state::<controller::Controller>()
            .archive(project_id)
            .map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_project_data_archive_path(
        handle: AppHandle,
        project_id: &str,
    ) -> Result<path::PathBuf, Error> {
        let project_id = project_id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        handle
            .state::<controller::Controller>()
            .data_archive(project_id)
            .map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_logs_archive_path(handle: AppHandle) -> Result<path::PathBuf, Error> {
        handle
            .state::<controller::Controller>()
            .logs_archive()
            .map_err(Into::into)
    }
}
