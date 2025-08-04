use but_api::{commands::git, App, NoParams};
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn git_remote_branches(
    app: State<App>,
    project_id: ProjectId,
) -> Result<Vec<RemoteRefname>, Error> {
    git::git_remote_branches(&app, git::GitRemoteBranchesParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn git_test_push(
    app: State<App>,
    project_id: ProjectId,
    remote_name: String,
    branch_name: String,
) -> Result<(), Error> {
    git::git_test_push(
        &app,
        git::GitTestPushParams {
            project_id,
            remote_name,
            branch_name,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn git_test_fetch(
    app: State<App>,
    project_id: ProjectId,
    remote_name: String,
    action: Option<String>,
) -> Result<(), Error> {
    git::git_test_fetch(
        &app,
        git::GitTestFetchParams {
            project_id,
            remote_name,
            action,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn git_index_size(app: State<App>, project_id: ProjectId) -> Result<usize, Error> {
    git::git_index_size(&app, git::GitIndexSizeParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn git_head(app: State<App>, project_id: ProjectId) -> Result<String, Error> {
    git::git_head(&app, git::GitHeadParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn delete_all_data(app: State<App>) -> Result<(), Error> {
    git::delete_all_data(&app, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn git_set_global_config(app: State<App>, key: String, value: String) -> Result<String, Error> {
    git::git_set_global_config(&app, git::GitSetGlobalConfigParams { key, value })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn git_remove_global_config(app: State<App>, key: String) -> Result<(), Error> {
    git::git_remove_global_config(&app, git::GitRemoveGlobalConfigParams { key })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug), level = "trace")]
pub fn git_get_global_config(app: State<App>, key: String) -> Result<Option<String>, Error> {
    git::git_get_global_config(&app, git::GitGetGlobalConfigParams { key })
}
