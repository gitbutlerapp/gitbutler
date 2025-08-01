use but_api::{commands::git, IpcContext, NoParams};
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_remote_branches(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<Vec<RemoteRefname>, Error> {
    git::git_remote_branches(&ipc_ctx, git::GitRemoteBranchesParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_test_push(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    remote_name: String,
    branch_name: String,
) -> Result<(), Error> {
    git::git_test_push(
        &ipc_ctx,
        git::GitTestPushParams {
            project_id,
            remote_name,
            branch_name,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_test_fetch(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    remote_name: String,
    action: Option<String>,
) -> Result<(), Error> {
    git::git_test_fetch(
        &ipc_ctx,
        git::GitTestFetchParams {
            project_id,
            remote_name,
            action,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_index_size(ipc_ctx: State<IpcContext>, project_id: ProjectId) -> Result<usize, Error> {
    git::git_index_size(&ipc_ctx, git::GitIndexSizeParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_head(ipc_ctx: State<IpcContext>, project_id: ProjectId) -> Result<String, Error> {
    git::git_head(&ipc_ctx, git::GitHeadParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn delete_all_data(ipc_ctx: State<IpcContext>) -> Result<(), Error> {
    git::delete_all_data(&ipc_ctx, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_set_global_config(
    ipc_ctx: State<IpcContext>,
    key: String,
    value: String,
) -> Result<String, Error> {
    git::git_set_global_config(&ipc_ctx, git::GitSetGlobalConfigParams { key, value })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_remove_global_config(ipc_ctx: State<IpcContext>, key: String) -> Result<(), Error> {
    git::git_remove_global_config(&ipc_ctx, git::GitRemoveGlobalConfigParams { key })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug), level = "trace")]
pub fn git_get_global_config(
    ipc_ctx: State<IpcContext>,
    key: String,
) -> Result<Option<String>, Error> {
    git::git_get_global_config(&ipc_ctx, git::GitGetGlobalConfigParams { key })
}
