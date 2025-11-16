use std::collections::HashMap;

use anyhow::{Context, Result};
use but_secret::Sensitive;
use but_settings::AppSettings;
use serde::{Deserialize, Serialize};

mod client;
pub mod pr;
pub use client::{CreatePullRequestParams, GitHubPrLabel, GitHubUser, PullRequest};
mod token;
pub use token::GithubAccountIdentifier;

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
    device_code: String,
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
    req_body.insert("device_code", device_code.as_str());
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
    account: &GithubAccountIdentifier,
    storage: &but_forge_storage::controller::Controller,
) -> Result<()> {
    token::delete_gh_access_token(account, storage).context("Failed to delete access token")
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
    account: &GithubAccountIdentifier,
    storage: &but_forge_storage::controller::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gh_access_token(account, storage)? {
        let gh = account
            .client(&access_token)
            .context("Failed to create GitHub client")?;
        let user = match gh.get_authenticated().await {
            Ok(user) => user,
            Err(client_err) => {
                // Check if this is a network error before converting to anyhow
                if is_network_error(&client_err) {
                    return Err(anyhow::Error::from(client_err).context(
                        but_error::Context::new_static(
                            but_error::Code::NetworkError,
                            "Unable to connect to GitHub.",
                        ),
                    ));
                }
                return Err(
                    anyhow::Error::from(client_err).context("Failed to get authenticated user")
                );
            }
        };
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

/// Check if an error is a network connectivity error.
///
/// This includes DNS resolution failures, connection timeouts, connection refused, etc.
fn is_network_error(err: &octorust::ClientError) -> bool {
    matches!(err, octorust::ClientError::ReqwestError(reqwest_err)
        if reqwest_err.is_timeout() || reqwest_err.is_connect() || reqwest_err.is_request())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

/// Check the validity of the stored credentials for the given GitHub account.
pub async fn check_credentials(
    account: &GithubAccountIdentifier,
    storage: &but_forge_storage::controller::Controller,
) -> Result<CredentialCheckResult> {
    if let Some(access_token) = token::get_gh_access_token(account, storage)? {
        let gh = account
            .client(&access_token)
            .context("Failed to create GitHub client")?;
        match gh.get_authenticated().await {
            Ok(_) => Ok(CredentialCheckResult::Valid),
            Err(_) => Ok(CredentialCheckResult::Invalid),
        }
    } else {
        Ok(CredentialCheckResult::NoCredentials)
    }
}

pub async fn list_known_github_accounts(
    storage: &but_forge_storage::controller::Controller,
) -> Result<Vec<token::GithubAccountIdentifier>> {
    let known_accounts = token::list_known_github_accounts(storage)
        .context("Failed to list known GitHub usernames")?;
    // Migrate the users from the previous storage method.
    if let Some(stored_gh_access_token) = gitbutler_user::forget_github_login_for_user()?
        && known_accounts.is_empty()
    {
        fetch_and_persist_oauth_user_data(&stored_gh_access_token, storage)
            .await
            .ok();

        let known_accounts = token::list_known_github_accounts(storage)
            .context("Failed to list known GitHub usernames")?;
        return Ok(known_accounts);
    }
    Ok(known_accounts)
}

pub fn clear_all_github_tokens(storage: &but_forge_storage::controller::Controller) -> Result<()> {
    token::clear_all_github_accounts(storage).context("Failed to clear all GitHub tokens")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_network_error_with_reqwest_timeout() {
        // Create a reqwest error by making an actual HTTP request that will timeout
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(1))
            .build()
            .unwrap();

        // Try to connect to a non-routable IP address (should timeout)
        let result = client.get("http://192.0.2.1:80").send();

        if let Err(reqwest_err) = result {
            let client_err = octorust::ClientError::ReqwestError(reqwest_err);
            assert!(
                is_network_error(&client_err),
                "Should detect timeout/connection errors"
            );
        } else {
            panic!("Expected a network error but request succeeded");
        }
    }

    #[test]
    fn test_is_network_error_with_connection_error() {
        // Create a reqwest error and wrap it in ClientError
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(1))
            .build()
            .unwrap();

        let result = client.get("http://192.0.2.1:80").send();

        if let Err(reqwest_err) = result {
            let client_err = octorust::ClientError::ReqwestError(reqwest_err);
            assert!(
                is_network_error(&client_err),
                "Should detect ClientError wrapping reqwest network errors"
            );
        } else {
            panic!("Expected a network error but request succeeded");
        }
    }

    #[test]
    fn test_is_not_network_error_http_error() {
        // HTTP errors (like 401) are not network errors
        let client_err = octorust::ClientError::HttpError {
            status: http::StatusCode::UNAUTHORIZED,
            headers: reqwest::header::HeaderMap::new(),
            error: "Unauthorized".to_string(),
        };
        assert!(
            !is_network_error(&client_err),
            "Should not detect HTTP status errors as network errors"
        );
    }

    #[test]
    fn test_is_not_network_error_rate_limit() {
        // Rate limit errors are not network errors
        let client_err = octorust::ClientError::RateLimited { duration: 60 };
        assert!(
            !is_network_error(&client_err),
            "Should not detect rate limit errors as network errors"
        );
    }
}
