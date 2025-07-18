use but_api::{
    commands::github::{self, Verification},
    IpcContext, NoParams,
};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub async fn init_device_oauth(ipc_ctx: State<'_, IpcContext>) -> Result<Verification, Error> {
    github::init_device_oauth(&ipc_ctx, NoParams {}).await
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub async fn check_auth_status(
    ipc_ctx: State<'_, IpcContext>,
    device_code: String,
) -> Result<String, Error> {
    github::check_auth_status(&ipc_ctx, github::CheckAuthStatusParams { device_code }).await
}
