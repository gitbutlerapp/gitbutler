use but_api::{commands::secret, IpcContext};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn secret_get_global(
    ipc_ctx: State<IpcContext>,
    handle: String,
) -> Result<Option<String>, Error> {
    secret::secret_get_global(&ipc_ctx, secret::SecretGetGlobalParams { handle })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx, secret), err(Debug), fields(secret = "<redacted>"))]
pub fn secret_set_global(
    ipc_ctx: State<IpcContext>,
    handle: String,
    secret: String,
) -> Result<(), Error> {
    secret::secret_set_global(&ipc_ctx, secret::SecretSetGlobalParams { handle, secret })
}
