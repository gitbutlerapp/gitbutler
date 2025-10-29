//! In place of commands.rs
use anyhow::Result;
use but_api_macros::api_cmd;
use but_github::{AuthStatusResponse, AuthenticatedUser, CheckAuthStatusParams, Verification};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{NoParams, error::Error};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

impl From<AuthStatusResponse> for AuthStatusResponseSensitive {
    fn from(
        AuthStatusResponse {
            access_token,
            login,
            name,
            email,
            host,
        }: AuthStatusResponse,
    ) -> Self {
        AuthStatusResponseSensitive {
            access_token: access_token.0,
            login,
            name,
            email,
            host,
        }
    }
}

pub async fn check_auth_status(
    params: CheckAuthStatusParams,
) -> Result<AuthStatusResponseSensitive, Error> {
    let storage = but_forge_storage::controller::Controller::from_path(but_path::app_data_dir()?);
    let status_result = but_github::check_auth_status(params, &storage).await;
    match status_result {
        Ok(status) => Ok(status.into()),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreGitHubPatParams {
    pub access_token: String,
}

pub async fn strore_github_pat(
    params: StoreGitHubPatParams,
) -> Result<AuthStatusResponseSensitive, Error> {
    let StoreGitHubPatParams { access_token } = params;
    let storage = but_forge_storage::controller::Controller::from_path(but_path::app_data_dir()?);
    let status_result = but_github::store_pat(&but_secret::Sensitive(access_token), &storage).await;
    match status_result {
        Ok(status) => Ok(status.into()),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreGitHubEnterprisePatParams {
    pub access_token: String,
    pub host: String,
}

pub async fn store_github_enterprise_pat(
    params: StoreGitHubEnterprisePatParams,
) -> Result<AuthStatusResponseSensitive, Error> {
    let StoreGitHubEnterprisePatParams { access_token, host } = params;
    let storage = but_forge_storage::controller::Controller::from_path(but_path::app_data_dir()?);
    let status_result =
        but_github::store_enterprise_pat(&host, &but_secret::Sensitive(access_token), &storage)
            .await;
    match status_result {
        Ok(status) => Ok(status.into()),
        Err(e) => Err(e.into()),
    }
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn forget_github_account(account: but_github::GithubAccountIdentifier) -> Result<(), Error> {
    let storage = but_forge_storage::controller::Controller::from_path(but_path::app_data_dir()?);
    but_github::forget_gh_access_token(&account, &storage).ok();
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn clear_all_github_tokens() -> Result<(), Error> {
    let storage = but_forge_storage::controller::Controller::from_path(but_path::app_data_dir()?);
    but_github::clear_all_github_tokens(&storage).map_err(Into::into)
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGhUserParams {
    pub account: but_github::GithubAccountIdentifier,
}

pub async fn get_gh_user(
    params: GetGhUserParams,
) -> Result<Option<AuthenticatedUserSensitive>, Error> {
    let GetGhUserParams { account } = params;
    let storage = but_forge_storage::controller::Controller::from_path(but_path::app_data_dir()?);
    but_github::get_gh_user(&account, &storage)
        .await
        .map(|res| res.map(Into::into))
        .map_err(Into::into)
}

pub async fn list_known_github_accounts() -> Result<Vec<but_github::GithubAccountIdentifier>, Error>
{
    let storage = but_forge_storage::controller::Controller::from_path(but_path::app_data_dir()?);
    but_github::list_known_github_accounts(&storage)
        .await
        .map_err(Into::into)
}
