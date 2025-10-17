use but_api::{
    NoParams,
    commands::github::{self},
    error::Error,
    github::{AuthStatusResponseSensitive, AuthenticatedUserSensitive, GetGhUserParams},
};
use but_github::{CheckAuthStatusParams, Verification};
use tracing::instrument;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn init_device_oauth() -> Result<Verification, Error> {
    github::init_device_oauth(NoParams {}).await
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn check_auth_status(device_code: String) -> Result<AuthStatusResponseSensitive, Error> {
    github::check_auth_status(CheckAuthStatusParams { device_code }).await
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn get_gh_user(username: String) -> Result<Option<AuthenticatedUserSensitive>, Error> {
    github::get_gh_user(GetGhUserParams { username }).await
}
