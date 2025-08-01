use but_api::{commands::remotes, IpcContext};
use gitbutler_project::ProjectId;
use gitbutler_repo::GitRemote;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn list_remotes(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<Vec<GitRemote>, Error> {
    remotes::list_remotes(&ipc_ctx, remotes::ListRemotesParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn add_remote(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    name: String,
    url: String,
) -> Result<(), Error> {
    remotes::add_remote(
        &ipc_ctx,
        remotes::AddRemoteParams {
            project_id,
            name,
            url,
        },
    )
}
