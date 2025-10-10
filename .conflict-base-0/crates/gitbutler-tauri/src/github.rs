use but_api::{
    commands::github::{self},
    NoParams,
};
use but_github::{CheckAuthStatusParams, Verification};
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn init_device_oauth() -> Result<Verification, Error> {
    github::init_device_oauth(NoParams {}).await
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn check_auth_status(device_code: String) -> Result<String, Error> {
    github::check_auth_status(CheckAuthStatusParams { device_code }).await
}
