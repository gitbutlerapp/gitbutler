use gitbutler_core::{git, projects::ProjectId};
use tauri::Manager;
use tracing::instrument;

use crate::app;
use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn git_remote_branches(
    handle: tauri::AppHandle,
    project_id: ProjectId,
) -> Result<Vec<git::RemoteRefname>, Error> {
    let app = handle.state::<app::App>();
    let branches = app.git_remote_branches(project_id)?;
    Ok(branches)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn git_test_push(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    remote_name: &str,
    branch_name: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    let helper = handle.state::<gitbutler_core::git::credentials::Helper>();
    Ok(app.git_test_push(
        project_id,
        remote_name,
        branch_name,
        &helper,
        // Run askpass, but don't pass any action
        Some(None),
    )?)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn git_test_fetch(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    remote_name: &str,
    action: Option<String>,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    let helper = handle.state::<gitbutler_core::git::credentials::Helper>();
    Ok(app.git_test_fetch(
        project_id,
        remote_name,
        &helper,
        Some(action.unwrap_or_else(|| "test".to_string())),
    )?)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn git_index_size(
    handle: tauri::AppHandle,
    project_id: ProjectId,
) -> Result<usize, Error> {
    let app = handle.state::<app::App>();
    Ok(app.git_index_size(project_id).expect("git index size"))
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn git_head(handle: tauri::AppHandle, project_id: ProjectId) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let head = app.git_head(project_id)?;
    Ok(head)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn delete_all_data(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.delete_all_data().await?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn mark_resolved(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    path: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.mark_resolved(project_id, path)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(_handle), err(Debug))]
pub async fn git_set_global_config(
    _handle: tauri::AppHandle,
    key: &str,
    value: &str,
) -> Result<String, Error> {
    let result = app::App::git_set_global_config(key, value)?;
    Ok(result)
}

#[tauri::command(async)]
#[instrument(skip(_handle), err(Debug))]
pub async fn git_get_global_config(
    _handle: tauri::AppHandle,
    key: &str,
) -> Result<Option<String>, Error> {
    let result = app::App::git_get_global_config(key)?;
    Ok(result)
}
