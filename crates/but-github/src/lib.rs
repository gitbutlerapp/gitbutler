use std::collections::HashMap;

use anyhow::{Context, Result};
use but_secret::Sensitive;
use but_settings::AppSettings;
use serde::{Deserialize, Serialize};

mod client;
pub mod pr;
pub use client::{CreatePullRequestParams, GitHubPrLabel, GitHubUser, PullRequest};
mod token;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Verification {
    pub user_code: String,
    pub device_code: String,
}

pub async fn init_device_oauth() -> Result<Verification> {
    let mut req_body = HashMap::new();
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let client_id = app_settings.github_oauth_app.oauth_client_id.clone();
    req_body.insert("client_id", client_id.as_str());
    req_body.insert("scope", "repo");

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let client = reqwest::Client::new();
    let res = client
        .post("https://github.com/login/device/code")
        .headers(headers)
        .json(&req_body)
        .send()
        .await
        .context("Failed to send request")?;

    let rsp_body = res.text().await.context("Failed to get response body")?;

    serde_json::from_str(&rsp_body).context("Failed to parse response body")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckAuthStatusParams {
    pub device_code: String,
}

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    /// The access token.
    /// This is only shared with the FrontEnd temporarily as we undergo the migration to having all API calls
    /// made to the forges from the Rustend.
    pub access_token: Sensitive<String>,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: Option<String>,
}

pub async fn check_auth_status(
    params: CheckAuthStatusParams,
    storage: &but_forge_storage::controller::Controller,
) -> Result<AuthStatusResponse> {
    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    struct AccessTokenContainer {
        access_token: String,
    }

    let mut req_body = HashMap::new();
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let client_id = app_settings.github_oauth_app.oauth_client_id.clone();
    req_body.insert("client_id", client_id.as_str());
    req_body.insert("device_code", params.device_code.as_str());
    req_body.insert("grant_type", "urn:ietf:params:oauth:grant-type:device_code");

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let client = reqwest::Client::new();
    let res = client
        .post("https://github.com/login/oauth/access_token")
        .headers(headers)
        .json(&req_body)
        .send()
        .await
        .context("Failed to send request")?;

    let rsp_body = res.text().await.context("Failed to get response body")?;

    let access_token = Sensitive(
        serde_json::from_str::<AccessTokenContainer>(&rsp_body)
            .map(|rsp_body| rsp_body.access_token)
            .context("Failed to parse response body")?,
    );

    let user = fetch_and_persist_oauth_user_data(&access_token, storage).await?;

    Ok(AuthStatusResponse {
        access_token,
        login: user.login,
        name: user.name,
        email: user.email,
        host: None,
    })
}

/// Fetch the authenticated user data from GitHub and persist the access token. (OAuth)
async fn fetch_and_persist_oauth_user_data(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gh = client::GitHubClient::new(access_token).context("Failed to create GitHub client")?;
    let user = gh
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gh_access_token(
        &token::GithubAccountIdentifier::oauth(&user.login),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

/// Store a PAT access token and fetch the associated user data.
pub async fn store_pat(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_pat_user_data(access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        login: user.login,
        name: user.name,
        email: user.email,
        host: None,
    })
}

/// Fetch the authenticated user data from GitHub and persist the access token. (PAT)
async fn fetch_and_persist_pat_user_data(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gh = client::GitHubClient::new(access_token).context("Failed to create GitHub client")?;
    let user = gh
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gh_access_token(
        &token::GithubAccountIdentifier::pat(&user.login),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

/// Store an Enterprise access token and fetch the associated user data.
pub async fn store_enterprise_pat(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_enterprise_user_data(host, access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        login: user.login,
        name: user.name,
        email: user.email,
        host: Some(host.to_owned()),
    })
}

/// Fetch the authenticated user data from GitHub and persist the access token. (Enterprise)
async fn fetch_and_persist_enterprise_user_data(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gh = client::GitHubClient::new_with_host_override(access_token, host)
        .context("Failed to create GitHub client")?;
    let user = gh
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gh_access_token(
        &token::GithubAccountIdentifier::enterprise(&user.login, host),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

pub fn forget_gh_access_token(
    login: &str,
    storage: &but_forge_storage::controller::Controller,
) -> Result<()> {
    token::delete_gh_access_token(&token::GithubAccountIdentifier::oauth(login), storage)
        .context("Failed to delete access token")
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub access_token: Sensitive<String>,
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

pub async fn get_gh_user(
    login: &str,
    storage: &but_forge_storage::controller::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) =
        token::get_gh_access_token(&token::GithubAccountIdentifier::oauth(login), storage)?
    {
        let gh =
            client::GitHubClient::new(&access_token).context("Failed to create GitHub client")?;
        let user = gh
            .get_authenticated()
            .await
            .context("Failed to get authenticated user")?;
        Ok(Some(AuthenticatedUser {
            access_token,
            login: user.login,
            avatar_url: user.avatar_url,
            name: user.name,
            email: user.email,
        }))
    } else {
        Ok(None)
    }
}

pub async fn list_known_github_usernames(
    storage: &but_forge_storage::controller::Controller,
) -> Result<Vec<String>> {
    let known_usernames = token::list_known_github_usernames(storage)
        .context("Failed to list known GitHub usernames")?;
    // Migrate the users from the previous storage method.
    if let Some(stored_gh_access_token) = gitbutler_user::forget_github_login_for_user()?
        && known_usernames.is_empty()
    {
        fetch_and_persist_oauth_user_data(&stored_gh_access_token, storage)
            .await
            .ok();
    }
    Ok(known_usernames)
}

pub fn clear_all_github_tokens(storage: &but_forge_storage::controller::Controller) -> Result<()> {
    token::clear_all_github_accounts(storage).context("Failed to clear all GitHub tokens")
}
