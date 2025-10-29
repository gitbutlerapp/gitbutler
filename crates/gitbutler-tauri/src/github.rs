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
#[instrument(err(Debug), skip(access_token), fields(access_token = "<redacted>"))]
pub async fn store_github_pat(access_token: String) -> Result<AuthStatusResponseSensitive, Error> {
    github::strore_github_pat(github::StoreGitHubPatParams { access_token }).await
}

#[tauri::command(async)]
#[instrument(err(Debug), skip(access_token), fields(access_token = "<redacted>"))]
pub async fn store_github_enterprise_pat(
    access_token: String,
    host: String,
) -> Result<AuthStatusResponseSensitive, Error> {
    github::store_github_enterprise_pat(github::StoreGitHubEnterprisePatParams {
        access_token,
        host,
    })
    .await
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn get_gh_user(
    account: but_github::GithubAccountIdentifier,
) -> Result<Option<AuthenticatedUserSensitive>, Error> {
    github::get_gh_user(GetGhUserParams { account }).await
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn list_known_github_accounts() -> Result<Vec<but_github::GithubAccountIdentifier>, Error>
{
    github::list_known_github_accounts().await
}
