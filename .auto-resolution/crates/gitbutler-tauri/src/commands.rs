use but_api::{commands::git, NoParams};
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_remote_branches(project_id: ProjectId) -> Result<Vec<RemoteRefname>, Error> {
    git::git_remote_branches(project_id)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_test_push(
    project_id: ProjectId,
    remote_name: String,
    branch_name: String,
) -> Result<(), Error> {
    git::git_test_push(project_id, remote_name, branch_name)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_test_fetch(
    project_id: ProjectId,
    remote_name: String,
    action: Option<String>,
) -> Result<(), Error> {
    git::git_test_fetch(project_id, remote_name, action)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_index_size(project_id: ProjectId) -> Result<usize, Error> {
    git::git_index_size(project_id)
}
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_head(project_id: ProjectId) -> Result<String, Error> {
    git::git_head(project_id)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn delete_all_data() -> Result<(), Error> {
    git::delete_all_data(NoParams {})
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_set_global_config(key: String, value: String) -> Result<String, Error> {
    git::git_set_global_config(key, value)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_remove_global_config(key: String) -> Result<(), Error> {
    git::git_remove_global_config(key)
}

#[tauri::command(async)]
#[instrument(err(Debug), level = "trace")]
pub fn git_get_global_config(key: String) -> Result<Option<String>, Error> {
    git::git_get_global_config(key)
}
