use but_api::{commands::open, IpcContext};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn open_url(ipc_ctx: State<'_, IpcContext>, url: String) -> Result<(), Error> {
    open::open_url(&ipc_ctx, open::OpenUrlParams { url })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn show_in_finder(ipc_ctx: State<'_, IpcContext>, path: String) -> Result<(), Error> {
    open::show_in_finder(&ipc_ctx, open::ShowInFinderParams { path })
}
