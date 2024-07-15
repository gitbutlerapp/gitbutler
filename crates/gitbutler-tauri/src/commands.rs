use crate::error::Error;
use crate::App;
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::credentials;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn git_remote_branches(
    app: State<'_, App>,
    project_id: ProjectId,
) -> Result<Vec<RemoteRefname>, Error> {
    Ok(app.git_remote_branches(project_id)?)
}

#[tauri::command(async)]
#[instrument(skip(app, helper), err(Debug))]
pub async fn git_test_push(
    app: State<'_, App>,
    helper: State<'_, credentials::Helper>,
    project_id: ProjectId,
    remote_name: &str,
    branch_name: &str,
) -> Result<(), Error> {
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
#[instrument(skip(app, helper), err(Debug))]
pub async fn git_test_fetch(
    app: State<'_, App>,
    helper: State<'_, credentials::Helper>,
    project_id: ProjectId,
    remote_name: &str,
    action: Option<String>,
) -> Result<(), Error> {
    Ok(app.git_test_fetch(
        project_id,
        remote_name,
        &helper,
        Some(action.unwrap_or_else(|| "test".to_string())),
    )?)
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn git_index_size(app: State<'_, App>, project_id: ProjectId) -> Result<usize, Error> {
    Ok(app.git_index_size(project_id).expect("git index size"))
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn git_head(app: State<'_, App>, project_id: ProjectId) -> Result<String, Error> {
    Ok(app.git_head(project_id)?)
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn delete_all_data(app: State<'_, App>) -> Result<(), Error> {
    app.delete_all_data().await?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn mark_resolved(
    app: State<'_, App>,
    project_id: ProjectId,
    path: &str,
) -> Result<(), Error> {
    app.mark_resolved(project_id, path)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn git_set_global_config(key: &str, value: &str) -> Result<String, Error> {
    Ok(App::git_set_global_config(key, value)?)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn git_remove_global_config(key: &str) -> Result<(), Error> {
    Ok(App::git_remove_global_config(key)?)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn git_get_global_config(key: &str) -> Result<Option<String>, Error> {
    Ok(App::git_get_global_config(key)?)
}
