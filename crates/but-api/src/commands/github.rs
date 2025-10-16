//! In place of commands.rs
use crate::{NoParams, error::Error};
use anyhow::Result;
use but_github::{AuthStatusResponse, AuthenticatedUser, CheckAuthStatusParams, Verification};
use but_settings::AppSettingsWithDiskSync;
use serde::Serialize;

pub async fn init_device_oauth(_params: NoParams) -> Result<Verification, Error> {
    but_github::init_device_oauth().await.map_err(Into::into)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResponseSensitive {
    pub access_token: String,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

impl From<AuthStatusResponse> for AuthStatusResponseSensitive {
    fn from(
        AuthStatusResponse {
            access_token,
            login,
            name,
            email,
        }: AuthStatusResponse,
    ) -> Self {
        AuthStatusResponseSensitive {
            access_token: access_token.0,
            login,
            name,
            email,
        }
    }
}

pub async fn check_auth_status(
    app_settings_sync: &AppSettingsWithDiskSync,
    params: CheckAuthStatusParams,
) -> Result<AuthStatusResponseSensitive, Error> {
    let status_result = but_github::check_auth_status(params).await;
    match status_result {
        Ok(status) => {
            app_settings_sync.add_known_github_username(&status.login)?;
            Ok(status.into())
        }
        Err(e) => Err(e.into()),
    }
}

pub fn forget_github_username(
    app_settings_sync: &AppSettingsWithDiskSync,
    login: String,
) -> Result<(), Error> {
    but_github::forget_gh_access_token(&login).ok();
    app_settings_sync
        .remove_known_github_username(&login)
        .map_err(Into::into)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUserSensitive {
    pub access_token: String,
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

impl From<AuthenticatedUser> for AuthenticatedUserSensitive {
    fn from(
        AuthenticatedUser {
            access_token,
            login,
            avatar_url,
            name,
            email,
        }: AuthenticatedUser,
    ) -> Self {
        AuthenticatedUserSensitive {
            access_token: access_token.0,
            login,
            avatar_url,
            name,
            email,
        }
    }
}

pub async fn get_gh_user(login: String) -> Result<Option<AuthenticatedUserSensitive>, Error> {
    but_github::get_gh_user(&login)
        .await
        .map(|res| res.map(Into::into))
        .map_err(Into::into)
}
