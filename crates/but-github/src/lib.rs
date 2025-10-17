use std::collections::HashMap;

use anyhow::{Context, Result};
use but_secret::Sensitive;
use but_settings::AppSettings;
use serde::{Deserialize, Serialize};

mod client;
pub mod pr;
pub use client::{GitHubPrLabel, GitHubUser, PullRequest};
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
    pub access_token: Sensitive<String>,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

pub async fn check_auth_status(params: CheckAuthStatusParams) -> Result<AuthStatusResponse> {
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

    let gh = client::GitHubClient::new(&access_token).context("Failed to create GitHub client")?;
    let user = gh
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;

    token::persist_gh_access_token(&user.login, &access_token)
        .context("Failed to persist access token")?;

    Ok(AuthStatusResponse {
        access_token,
        login: user.login,
        name: user.name,
        email: user.email,
    })
}

pub fn forget_gh_access_token(login: &str) -> Result<()> {
    token::delete_gh_access_token(login).context("Failed to delete access token")
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub access_token: Sensitive<String>,
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

pub async fn get_gh_user(login: &str) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gh_access_token(login)? {
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

pub fn list_known_github_usernames() -> Result<Vec<String>> {
    token::list_known_github_usernames().context("Failed to list known GitHub usernames")
}

pub fn clear_all_github_tokens() -> Result<()> {
    token::clear_all_github_tokens().context("Failed to clear all GitHub tokens")
}
