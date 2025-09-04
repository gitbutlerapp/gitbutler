use but_api::commands::secret;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn secret_get_global(handle: String) -> Result<Option<String>, Error> {
    secret::secret_get_global(secret::SecretGetGlobalParams { handle })
}

#[tauri::command(async)]
#[instrument(skip(secret), err(Debug), fields(secret = "<redacted>"))]
pub fn secret_set_global(handle: String, secret: String) -> Result<(), Error> {
    secret::secret_set_global(secret::SecretSetGlobalParams { handle, secret })
}
