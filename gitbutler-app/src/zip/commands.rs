use std::path;

use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::error::{Code, UserError};

use super::controller;

impl From<controller::ArchiveError> for UserError {
    fn from(error: controller::ArchiveError) -> Self {
        match error {
            controller::ArchiveError::GetProject(error) => error.into(),
            controller::ArchiveError::Other(error) => {
                tracing::error!(?error, "failed to archive project");
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_project_archive_path(
    handle: AppHandle,
    project_id: &str,
) -> Result<path::PathBuf, UserError> {
    let project_id = project_id.parse().map_err(|_| UserError::User {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;
    handle
        .state::<controller::Controller>()
        .archive(&project_id)
        .map_err(Into::into)
}

impl From<controller::DataArchiveError> for UserError {
    fn from(value: controller::DataArchiveError) -> Self {
        match value {
            controller::DataArchiveError::GetProject(error) => error.into(),
            controller::DataArchiveError::Other(error) => {
                tracing::error!(?error, "failed to archive project data");
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_project_data_archive_path(
    handle: AppHandle,
    project_id: &str,
) -> Result<path::PathBuf, UserError> {
    let project_id = project_id.parse().map_err(|_| UserError::User {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;
    handle
        .state::<controller::Controller>()
        .data_archive(&project_id)
        .map_err(Into::into)
}

impl From<controller::LogsArchiveError> for UserError {
    fn from(error: controller::LogsArchiveError) -> Self {
        match error {
            controller::LogsArchiveError::Other(error) => {
                tracing::error!(?error, "failed to archive logs");
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_logs_archive_path(handle: AppHandle) -> Result<path::PathBuf, UserError> {
    handle
        .state::<controller::Controller>()
        .logs_archive()
        .map_err(Into::into)
}
