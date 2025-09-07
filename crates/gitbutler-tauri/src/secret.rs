use but_api::commands::secret;

use but_api::error::Error;

#[tauri::command(async)]
pub fn secret_get_global(handle: String) -> Result<Option<String>, Error> {
    secret::secret_get_global(handle)
}

#[tauri::command(async)]
pub fn secret_set_global(handle: String, secret: String) -> Result<(), Error> {
    secret::secret_set_global(handle, secret)
}
