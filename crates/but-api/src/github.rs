//! In place of commands.rs
use anyhow::Result;
use but_api_macros::{api_cmd, api_cmd_tauri};
use but_github::{AuthStatusResponse, AuthenticatedUser, Verification};
use but_secret::Sensitive;
use tracing::instrument;

use crate::json::Error;

pub mod json {
    use but_github::{AuthStatusResponse, AuthenticatedUser};
    use serde::Serialize;

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
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub async fn init_device_oauth() -> Result<Verification> {
    but_github::init_device_oauth().await
}

#[api_cmd_tauri(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn check_auth_status(device_code: String) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::check_auth_status(device_code, &storage).await
}

#[api_cmd_tauri(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_github_pat(access_token: Sensitive<String>) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::store_pat(&access_token, &storage).await
}

#[api_cmd_tauri(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_github_enterprise_pat(
    access_token: Sensitive<String>,
    host: String,
) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::store_enterprise_pat(&host, &access_token, &storage).await
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn forget_github_account(account: but_github::GithubAccountIdentifier) -> Result<(), Error> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::forget_gh_access_token(&account, &storage).ok();
    Ok(())
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn clear_all_github_tokens() -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::clear_all_github_tokens(&storage)
}

#[api_cmd_tauri(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gh_user(
    account: but_github::GithubAccountIdentifier,
) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::get_gh_user(&account, &storage).await
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub async fn list_known_github_accounts() -> Result<Vec<but_github::GithubAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::list_known_github_accounts(&storage).await
}

#[instrument(err(Debug))]
pub async fn check_github_credentials(
    account: but_github::GithubAccountIdentifier,
) -> Result<but_github::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::check_credentials(&account, &storage).await
}
