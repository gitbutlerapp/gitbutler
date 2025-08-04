use but_api::{
    commands::github::{self, Verification},
    App, NoParams,
};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn init_device_oauth(app: State<'_, App>) -> Result<Verification, Error> {
    github::init_device_oauth(&app, NoParams {}).await
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn check_auth_status(app: State<'_, App>, device_code: String) -> Result<String, Error> {
    github::check_auth_status(&app, github::CheckAuthStatusParams { device_code }).await
}
