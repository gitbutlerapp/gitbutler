use but_api::{commands::secret, App};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn secret_get_global(app: State<App>, handle: String) -> Result<Option<String>, Error> {
    secret::secret_get_global(&app, secret::SecretGetGlobalParams { handle })
}

#[tauri::command(async)]
#[instrument(skip(app, secret), err(Debug), fields(secret = "<redacted>"))]
pub fn secret_set_global(app: State<App>, handle: String, secret: String) -> Result<(), Error> {
    secret::secret_set_global(&app, secret::SecretSetGlobalParams { handle, secret })
}
