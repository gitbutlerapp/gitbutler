pub mod commands {
    use gitbutler_core::keys::{controller, PublicKey};
    use tauri::Manager;
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_public_key(handle: tauri::AppHandle) -> Result<PublicKey, Error> {
        handle
            .state::<controller::Controller>()
            .get_or_create()
            .map(|key| key.public_key())
            .map_err(Into::into)
    }
}
