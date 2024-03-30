use std::{collections::HashMap, path};

use anyhow::Context;
use gitbutler::{
    error::{Code, Error},
    gb_repository, git, project_repository, projects, reader,
    sessions::SessionId,
    users,
};
use tauri::Manager;
use tracing::instrument;

use crate::{app, watcher};

impl From<app::Error> for Error {
    fn from(value: app::Error) -> Self {
        match value {
            app::Error::GetProject(error) => Error::from(error),
            app::Error::ProjectRemote(error) => Error::from(error),
            app::Error::OpenProjectRepository(error) => Error::from(error),
            app::Error::Other(error) => {
                tracing::error!(?error);
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_session_files(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<&path::Path>>,
) -> Result<HashMap<path::PathBuf, reader::Content>, Error> {
    let app = handle.state::<app::App>();
    let session_id: SessionId = session_id.parse().map_err(|_| Error::UserError {
        message: "Malformed session id".to_string(),
        code: Code::Validation,
    })?;
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let files = app.list_session_files(&project_id, &session_id, paths.as_deref())?;
    Ok(files)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_remote_branches(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<git::RemoteRefname>, Error> {
    let app = handle.state::<app::App>();
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branches = app.git_remote_branches(&project_id)?;
    Ok(branches)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_test_push(
    handle: tauri::AppHandle,
    project_id: &str,
    remote_name: &str,
    branch_name: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    let helper = handle.state::<gitbutler::git::credentials::Helper>();
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let askpass_broker = handle
        .state::<gitbutler::askpass::AskpassBroker>()
        .inner()
        .clone();
    app.git_test_push(
        &project_id,
        remote_name,
        branch_name,
        &helper,
        Some((askpass_broker, None)),
    )
    .map_err(|e| Error::UserError {
        code: Code::Unknown,
        message: e.to_string(),
    })
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_test_fetch(
    handle: tauri::AppHandle,
    project_id: &str,
    remote_name: &str,
    action: Option<String>,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    let helper = handle.state::<gitbutler::git::credentials::Helper>();
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let askpass_broker = handle
        .state::<gitbutler::askpass::AskpassBroker>()
        .inner()
        .clone();
    app.git_test_fetch(
        &project_id,
        remote_name,
        &helper,
        Some((askpass_broker, action.unwrap_or_else(|| "test".to_string()))),
    )
    .map_err(|e| Error::UserError {
        code: Code::Unknown,
        message: e.to_string(),
    })
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_index_size(handle: tauri::AppHandle, project_id: &str) -> Result<usize, Error> {
    let app = handle.state::<app::App>();
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    Ok(app.git_index_size(&project_id).expect("git index size"))
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_head(handle: tauri::AppHandle, project_id: &str) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let head = app.git_head(&project_id)?;
    Ok(head)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn delete_all_data(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.delete_all_data().await?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn mark_resolved(
    handle: tauri::AppHandle,
    project_id: &str,
    path: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    app.mark_resolved(&project_id, path)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(_handle))]
pub async fn git_set_global_config(
    _handle: tauri::AppHandle,
    key: &str,
    value: &str,
) -> Result<String, Error> {
    let result = app::App::git_set_global_config(key, value)?;
    Ok(result)
}

#[tauri::command(async)]
#[instrument(skip(_handle))]
pub async fn git_get_global_config(
    _handle: tauri::AppHandle,
    key: &str,
) -> Result<Option<String>, Error> {
    let result = app::App::git_get_global_config(key)?;
    Ok(result)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn project_flush_and_push(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    let project_id = id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;

    let users = handle.state::<users::Controller>().inner().clone();
    let projects = handle.state::<projects::Controller>().inner().clone();
    let local_data_dir = handle
        .path_resolver()
        .app_data_dir()
        .context("failed to get app data dir")?;

    let project = projects.get(&project_id).context("failed to get project")?;
    let user = users.get_user()?;
    let project_repository = project_repository::Repository::open(&project)?;
    let gb_repo =
        gb_repository::Repository::open(&local_data_dir, &project_repository, user.as_ref())
            .context("failed to open repository")?;

    if let Some(current_session) = gb_repo
        .get_current_session()
        .context("failed to get current session")?
    {
        let watcher = handle.state::<watcher::Watchers>();
        watcher
            .post(watcher::Event::Flush(project_id, current_session))
            .await
            .context("failed to post flush event")?;
    }

    Ok(())
}
