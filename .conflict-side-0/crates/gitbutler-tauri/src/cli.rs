use but_api::{commands::cli, IpcContext, NoParams};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn install_cli(ipc_ctx: State<'_, IpcContext>) -> Result<(), Error> {
    cli::install_cli(&ipc_ctx, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn cli_path(ipc_ctx: State<'_, IpcContext>) -> Result<String, Error> {
    cli::cli_path(&ipc_ctx, NoParams {})
}
