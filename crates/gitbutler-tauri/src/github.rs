use but_api::{
    commands::github::{self, Verification},
    NoParams,
};

use but_api::error::Error;

#[tauri::command(async)]
pub async fn init_device_oauth() -> Result<Verification, Error> {
    github::init_device_oauth(NoParams {}).await
}

#[tauri::command(async)]
pub async fn check_auth_status(device_code: String) -> Result<String, Error> {
    github::check_auth_status(device_code).await
}
