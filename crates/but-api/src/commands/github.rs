//! In place of commands.rs
use anyhow::Result;
use but_github::{AuthStatusResponse, AuthenticatedUser, CheckAuthStatusParams, Verification};
use but_settings::AppSettingsWithDiskSync;

use crate::{NoParams, error::Error};

pub async fn init_device_oauth(_params: NoParams) -> Result<Verification, Error> {
    but_github::init_device_oauth().await.map_err(Into::into)
}

pub async fn check_auth_status(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: CheckAuthStatusParams,
) -> Result<AuthStatusResponse, Error> {
    let status_result = but_github::check_auth_status(params).await;
    match status_result {
        Ok(status) => {
            app_settings_sync.add_known_github_username(&status.login)?;
            Ok(status)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn forget_github_username(
    app_settings_sync: &AppSettingsWithDiskSync,
    login: String,
) -> Result<(), Error> {
    but_github::forget_gh_access_token(&login)
        .map_err(Into::into)
        .and_then(|_| {
            app_settings_sync
                .remove_known_github_username(&login)
                .map_err(Into::into)
        })
}

pub async fn get_gh_user(login: String) -> Result<Option<AuthenticatedUser>, Error> {
    but_github::get_gh_user(&login).await.map_err(Into::into)
}
