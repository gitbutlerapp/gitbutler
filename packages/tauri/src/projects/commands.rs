use std::path;

use tauri::Manager;
use tracing::instrument;

use crate::{
    error::{Code, Error},
    projects,
};

use super::controller::{self, Controller};

impl From<controller::UpdateError> for Error {
    fn from(value: controller::UpdateError) -> Self {
        match value {
            controller::UpdateError::NotFound => Error::UserError {
                code: Code::Projects,
                message: "Project not found".into(),
            },
            controller::UpdateError::Other(error) => {
                tracing::error!(?error, "failed to update project");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn update_project(
    handle: tauri::AppHandle,
    project: projects::UpdateRequest,
) -> Result<projects::Project, Error> {
    handle
        .state::<Controller>()
        .update(&project)
        .map_err(Into::into)
}

impl From<controller::AddError> for Error {
    fn from(value: controller::AddError) -> Self {
        match value {
            controller::AddError::NotAGitRepository => Error::UserError {
                code: Code::Projects,
                message: "Must be a git directory".to_string(),
            },
            controller::AddError::AlreadyExists => Error::UserError {
                code: Code::Projects,
                message: "Project already exists".to_string(),
            },
            controller::AddError::NotADirectory => Error::UserError {
                code: Code::Projects,
                message: "Not a directory".to_string(),
            },
            controller::AddError::PathNotFound => Error::UserError {
                code: Code::Projects,
                message: "Path not found".to_string(),
            },
            controller::AddError::Other(error) => {
                tracing::error!(?error, "failed to add project");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn add_project(
    handle: tauri::AppHandle,
    path: &path::Path,
) -> Result<projects::Project, Error> {
    handle.state::<Controller>().add(path).map_err(Into::into)
}

impl From<controller::GetError> for Error {
    fn from(value: controller::GetError) -> Self {
        match value {
            controller::GetError::NotFound => Error::UserError {
                code: Code::Projects,
                message: "Project not found".into(),
            },
            controller::GetError::Other(error) => {
                tracing::error!(?error, "failed to get project");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_project(handle: tauri::AppHandle, id: &str) -> Result<projects::Project, Error> {
    handle.state::<Controller>().get(id).map_err(Into::into)
}

impl From<controller::ListError> for Error {
    fn from(value: controller::ListError) -> Self {
        match value {
            controller::ListError::Other(error) => {
                tracing::error!(?error, "failed to list projects");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_projects(handle: tauri::AppHandle) -> Result<Vec<projects::Project>, Error> {
    handle.state::<Controller>().list().map_err(Into::into)
}

impl From<controller::DeleteError> for Error {
    fn from(value: controller::DeleteError) -> Self {
        match value {
            controller::DeleteError::Other(error) => {
                tracing::error!(?error, "failed to delete project");
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn delete_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    handle.state::<Controller>().delete(id).map_err(Into::into)
}
