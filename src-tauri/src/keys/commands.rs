use timed::timed;

use crate::error::Error;

use super::{controller::Controller, PublicKey};

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn get_public_key(handle: tauri::AppHandle) -> Result<PublicKey, Error> {
    let controller = Controller::try_from(handle)?;
    let key = controller.get_or_create()?;
    Ok(key.public_key())
}
