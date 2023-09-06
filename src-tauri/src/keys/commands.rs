use tracing::instrument;

use crate::error::Error;

use super::{storage::Storage, PublicKey};

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_public_key(handle: tauri::AppHandle) -> Result<PublicKey, Error> {
    let controller = Storage::from(&handle);
    let key = controller.get_or_create()?;
    Ok(key.public_key())
}
