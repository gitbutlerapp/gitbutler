use but_api::{commands::config, IpcContext};
use but_core::settings::git::ui::GitConfigSettings;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn get_gb_config(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<GitConfigSettings, Error> {
    config::get_gb_config(&ipc_ctx, config::GetGbConfigParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn set_gb_config(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    config: GitConfigSettings,
) -> Result<(), Error> {
    config::set_gb_config(&ipc_ctx, config::SetGbConfigParams { project_id, config })
}
