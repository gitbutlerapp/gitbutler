use but_api::{commands::remotes, App};
use gitbutler_project::ProjectId;
use gitbutler_repo::GitRemote;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn list_remotes(app: State<App>, project_id: ProjectId) -> Result<Vec<GitRemote>, Error> {
    remotes::list_remotes(&app, remotes::ListRemotesParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn add_remote(
    app: State<App>,
    project_id: ProjectId,
    name: String,
    url: String,
) -> Result<(), Error> {
    remotes::add_remote(
        &app,
        remotes::AddRemoteParams {
            project_id,
            name,
            url,
        },
    )
}
