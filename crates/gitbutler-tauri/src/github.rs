use but_api::{
    NoParams,
    commands::github::{self},
    error::Error,
    github::{AuthStatusResponseSensitive, AuthenticatedUserSensitive, GetGhUserParams},
};
use but_github::{CheckAuthStatusParams, Verification};
use but_settings::AppSettingsWithDiskSync;
use tauri::State;
use tracing::instrument;

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
) -> Result<AuthStatusResponseSensitive, Error> {
    github::check_auth_status(&app_settings_sync, CheckAuthStatusParams { device_code }).await
}

#[tauri::command(async)]
#[instrument(skip(app_settings_sync), err(Debug))]
pub async fn forget_github_username(
    app_settings_sync: State<'_, AppSettingsWithDiskSync>,
    username: String,
) -> Result<(), Error> {
    github::forget_github_username(&app_settings_sync, username)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn get_gh_user(username: String) -> Result<Option<AuthenticatedUserSensitive>, Error> {
    github::get_gh_user(GetGhUserParams { username }).await
}
