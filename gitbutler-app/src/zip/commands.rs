#![allow(clippy::used_underscore_binding)]
use std::path;

use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::error::{Code, Error};

use super::controller;

impl From<controller::ArchiveError> for Error {
    fn from(error: controller::ArchiveError) -> Self {
        match error {
            controller::ArchiveError::GetProject(error) => error.into(),
            controller::ArchiveError::Other(error) => {
                tracing::error!(?error, "failed to archive project");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_project_archive_path(
    handle: AppHandle,
    project_id: &str,
) -> Result<path::PathBuf, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;
    handle
        .state::<controller::Controller>()
        .archive(&project_id)
        .map_err(Into::into)
}

impl From<controller::DataArchiveError> for Error {
    fn from(value: controller::DataArchiveError) -> Self {
        match value {
            controller::DataArchiveError::GetProject(error) => error.into(),
            controller::DataArchiveError::Other(error) => {
                tracing::error!(?error, "failed to archive project data");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_project_data_archive_path(
    handle: AppHandle,
    project_id: &str,
) -> Result<path::PathBuf, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;
    handle
        .state::<controller::Controller>()
        .data_archive(&project_id)
        .map_err(Into::into)
}

impl From<controller::LogsArchiveError> for Error {
    fn from(error: controller::LogsArchiveError) -> Self {
        match error {
            controller::LogsArchiveError::Other(error) => {
                tracing::error!(?error, "failed to archive logs");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_logs_archive_path(handle: AppHandle) -> Result<path::PathBuf, Error> {
    handle
        .state::<controller::Controller>()
        .logs_archive()
        .map_err(Into::into)
}
