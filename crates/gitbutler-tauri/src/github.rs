use but_api::{
    commands::github::{self},
    NoParams,
};
use but_github::{AuthStatusResponse, CheckAuthStatusParams, Verification};
use but_settings::AppSettingsWithDiskSync;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn init_device_oauth() -> Result<Verification, Error> {
    github::init_device_oauth(NoParams {}).await
}

#[tauri::command(async)]
#[instrument(skip(app_settings_sync), err(Debug))]
pub async fn check_auth_status(
    app_settings_sync: State<'_, AppSettingsWithDiskSync>,
    device_code: String,
) -> Result<AuthStatusResponse, Error> {
    github::check_auth_status(&app_settings_sync, CheckAuthStatusParams { device_code }).await
}
