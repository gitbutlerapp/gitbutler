use tauri::Manager;
use tracing::instrument;

use crate::error::UserError;

use super::{controller, PublicKey};

impl From<controller::GetOrCreateError> for UserError {
    fn from(value: controller::GetOrCreateError) -> Self {
        match value {
            controller::GetOrCreateError::Other(error) => {
                tracing::error!(?error, "failed to get or create key");
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_public_key(handle: tauri::AppHandle) -> Result<PublicKey, UserError> {
    handle
        .state::<controller::Controller>()
        .get_or_create()
        .map(|key| key.public_key())
        .map_err(Into::into)
}
