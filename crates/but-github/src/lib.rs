use std::collections::HashMap;

use anyhow::{Context as _, Result};
use but_secret::Sensitive;
use but_settings::AppSettings;
use serde::{Deserialize, Serialize};

pub mod checks;
mod client;
mod graphql;
pub mod pr;
pub mod stacks;
pub use client::{
    AutoMergeEnableParams, AutoMergeState, CheckRun, CommentReactions, CreatePullRequestParams,
    GitHubClient, GitHubPrLabel, GitHubRepoPermissions, GitHubRepository, GitHubUser, MergeMethod,
    MergePullRequestParams, PullRequest, PullRequestComment, PullRequestMergeStatus,
    PullRequestReview, PullRequestReviewThread, PullRequestReviewThreadComment,
    PullRequestTimelineEvent, PullRequestTimelineEventKind, Reaction,
    SetPullRequestAutoMergeParams, SetPullRequestDraftStateParams, UpdatePullRequestParams,
};
mod token;
pub use token::GithubAccountIdentifier;

#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Verification {
    pub user_code: String,
    pub device_code: String,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(Verification);

/// Detect GitHub's OAuth error shape (e.g. `device_flow_disabled`, `authorization_pending`) before falling back to the expected payload, so the real cause surfaces instead of a generic "missing field" serde error.
fn parse_github_oauth_response<T: serde::de::DeserializeOwned>(body: &str) -> Result<T> {
    let value: serde_json::Value =
        serde_json::from_str(body).context("Response body was not valid JSON")?;
    if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
        let description = value
            .get("error_description")
            .and_then(serde_json::Value::as_str);
        let err = anyhow::anyhow!(
            "GitHub returned an error: {} ({})",
            error,
            description.unwrap_or("no description"),
        );
        return Err(match device_flow_context(error) {
            Some(context) => err.context(context),
            None => err,
        });
    }
    serde_json::from_value(value).context("Response body did not match expected schema")
}

/// A static, code-bearing context for terminal device-flow statuses, so the API serializes
/// guidance instead of GitHub's description. Pending statuses stay as they are: callers poll
/// through them, and the provider text remains the inner error for the CLI and Lite.
fn device_flow_context(status: &str) -> Option<but_error::Context> {
    use but_error::{Code, Context};
    match status {
        "authorization_pending" | "slow_down" => None,
        "expired_token" => Some(Context::new_static(
            Code::GitHubDeviceCodeExpired,
            "The GitHub device code has expired",
        )),
        "access_denied" => Some(Context::new_static(
            Code::GitHubDeviceAccessDenied,
            "Authorization was denied on GitHub",
        )),
        _ => Some(Context::new_static(
            Code::GitHubDeviceFlowRejected,
            "GitHub rejected the device authorization request",
        )),
    }
}

pub async fn init_github_device_oauth() -> Result<Verification> {
    let mut req_body = HashMap::new();
    let app_settings = AppSettings::load_from_default_path_creating_without_customization()?;
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

    parse_github_oauth_response(&rsp_body)
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

pub async fn check_github_auth_status(
    device_code: String,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    struct AccessTokenContainer {
        access_token: String,
    }

    let mut req_body = HashMap::new();
    let app_settings = AppSettings::load_from_default_path_creating_without_customization()?;
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

    let access_token =
        Sensitive(parse_github_oauth_response::<AccessTokenContainer>(&rsp_body)?.access_token);

    let user = fetch_and_persist_oauth_user_data(&access_token, storage).await?;

    Ok(AuthStatusResponse {
        access_token,
        login: user.login,
        name: user.name,
        email: user.email,
        host: None,
    })
}

/// Cache the user profile so it's available offline.
fn cache_user_profile(
    account: &GithubAccountIdentifier,
    user: &client::AuthenticatedUser,
    storage: &but_forge_storage::Controller,
) {
    let profile = but_forge_storage::settings::CachedProfile {
        avatar_url: user.avatar_url.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
    };
    let key = account.cache_key();
    let existing = storage.cached_profile(&key).ok().flatten();
    if existing.as_ref() == Some(&profile) {
        return;
    }
    if let Err(err) = storage.set_cached_profile(&key, Some(profile)) {
        tracing::warn!(?account, "Failed to update cached GitHub profile: {err}");
    }
}

/// Fetch the authenticated user data from GitHub and persist the access token. (OAuth)
async fn fetch_and_persist_oauth_user_data(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gh = client::GitHubClient::new(access_token).context("Failed to create GitHub client")?;
    let user = gh
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    let account_id = token::GithubAccountIdentifier::oauth(&user.login);
    token::persist_gh_access_token(&account_id, access_token, storage)
        .context("Failed to persist access token")?;
    cache_user_profile(&account_id, &user, storage);
    Ok(user)
}

/// Store a PAT access token and fetch the associated user data.
pub async fn store_pat(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
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
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gh = client::GitHubClient::new(access_token).context("Failed to create GitHub client")?;
    let user = gh
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    let account_id = token::GithubAccountIdentifier::pat(&user.login);
    token::persist_gh_access_token(&account_id, access_token, storage)
        .context("Failed to persist access token")?;
    cache_user_profile(&account_id, &user, storage);
    Ok(user)
}

/// Store an Enterprise access token and fetch the associated user data.
pub async fn store_enterprise_pat(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
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
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gh = client::GitHubClient::new_with_host_override(access_token, host)
        .context("Failed to create GitHub client")?;
    let user = gh
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    let account_id = token::GithubAccountIdentifier::enterprise(&user.login, host);
    token::persist_gh_access_token(&account_id, access_token, storage)
        .context("Failed to persist access token")?;
    cache_user_profile(&account_id, &user, storage);
    Ok(user)
}

pub fn forget_gh_access_token(
    account: &GithubAccountIdentifier,
    storage: &but_forge_storage::Controller,
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
    storage: &but_forge_storage::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gh_access_token(account, storage)? {
        let gh = account
            .client(&access_token)
            .context("Failed to create GitHub client")?;
        match gh.get_authenticated().await {
            Ok(user) => {
                cache_user_profile(account, &user, storage);
                Ok(Some(AuthenticatedUser {
                    access_token,
                    login: user.login,
                    avatar_url: user.avatar_url,
                    name: user.name,
                    email: user.email,
                }))
            }
            Err(client_err) => {
                let cache_key = account.cache_key();
                // Check if this is a network error — return cached data if available.
                if let Some(reqwest_err) = client_err.downcast_ref::<reqwest::Error>()
                    && is_network_error(reqwest_err)
                {
                    match storage.cached_profile(&cache_key) {
                        Ok(Some(cached)) => {
                            return Ok(Some(AuthenticatedUser {
                                access_token,
                                login: account.username().to_owned(),
                                avatar_url: cached.avatar_url,
                                name: cached.name,
                                email: cached.email,
                            }));
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::warn!("Failed to read cached GitHub profile: {err}");
                        }
                    }
                    return Err(client_err.context(but_error::Context::new_static(
                        but_error::Code::NetworkError,
                        "Unable to connect to GitHub.",
                    )));
                }
                // Check if this is an auth error (401/403) — clear cached profile.
                if let Some(http_err) = client_err.downcast_ref::<client::HttpStatusError>()
                    && matches!(
                        http_err.status,
                        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
                    )
                    && let Err(err) = storage.set_cached_profile(&cache_key, None)
                {
                    tracing::warn!("Failed to clear cached GitHub profile: {err}");
                }
                Err(client_err.context("Failed to get authenticated user"))
            }
        }
    } else {
        Ok(None)
    }
}

/// Check if an error is a network connectivity error.
///
/// This includes DNS resolution failures, connection timeouts, connection
/// refused, and connections dropped while the response body was being read.
/// reqwest wraps both body I/O failures and malformed payloads as the same
/// decode kind, so the source chain decides: a serde cause means the payload
/// was malformed, anything else means the transport failed mid-response.
pub(crate) fn is_network_error(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() || err.is_request() {
        return true;
    }
    if !err.is_decode() {
        return false;
    }
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        if cause.downcast_ref::<serde_json::Error>().is_some() {
            return false;
        }
        source = cause.source();
    }
    true
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

/// Check the validity of the stored credentials for the given GitHub account.
pub async fn check_credentials(
    account: &GithubAccountIdentifier,
    storage: &but_forge_storage::Controller,
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

pub fn list_known_github_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<token::GithubAccountIdentifier>> {
    token::list_known_github_accounts(storage).context("Failed to list known GitHub usernames")
}

pub fn clear_all_github_tokens(storage: &but_forge_storage::Controller) -> Result<()> {
    token::clear_all_github_accounts(storage).context("Failed to clear all GitHub tokens")
}

/// JSON serialization types for GitHub API responses.
///
/// This module contains serializable versions of GitHub authentication types
/// that expose sensitive data (like access tokens) as plain strings for API responses.
pub mod json {
    use serde::Serialize;

    use crate::{AuthStatusResponse, AuthenticatedUser};

    /// Serializable version of [`AuthStatusResponse`], without the access token.
    ///
    /// The credential is stored by the backend as part of the call, so the caller is told
    /// who authenticated and nothing more. Field names are camelCase for JSON.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct GithubAuthStatusResponse {
        pub login: String,
        pub name: Option<String>,
        pub email: Option<String>,
        /// The enterprise or self-hosted host, when there is one.
        pub host: Option<String>,
    }

    impl From<AuthStatusResponse> for GithubAuthStatusResponse {
        fn from(
            AuthStatusResponse {
                login,
                name,
                email,
                host,
                ..
            }: AuthStatusResponse,
        ) -> Self {
            GithubAuthStatusResponse {
                login,
                name,
                email,
                host,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(GithubAuthStatusResponse);

    /// Serializable version of [`AuthenticatedUser`] with exposed access token.
    ///
    /// This struct represents an authenticated GitHub user with their credentials
    /// exposed as plain strings for API responses. Field names are converted to camelCase for JSON.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct GithubAuthenticatedUserSensitive {
        /// The GitHub access token as a plain string (sensitive data).
        pub access_token: String,
        /// The GitHub username/login.
        pub login: String,
        /// The URL to the user's avatar image, if available.
        pub avatar_url: Option<String>,
        /// The user's display name, if available.
        pub name: Option<String>,
        /// The user's email address, if available.
        pub email: Option<String>,
    }

    impl From<AuthenticatedUser> for GithubAuthenticatedUserSensitive {
        fn from(
            AuthenticatedUser {
                access_token,
                login,
                avatar_url,
                name,
                email,
            }: AuthenticatedUser,
        ) -> Self {
            GithubAuthenticatedUserSensitive {
                access_token: access_token.0,
                login,
                avatar_url,
                name,
                email,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(GithubAuthenticatedUserSensitive);
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
            assert!(
                is_network_error(&reqwest_err),
                "Should detect timeout/connection errors"
            );
        } else {
            panic!("Expected a network error but request succeeded");
        }
    }

    #[test]
    fn test_is_network_error_with_connection_error() {
        // Create a reqwest error
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(1))
            .build()
            .unwrap();

        let result = client.get("http://192.0.2.1:80").send();

        if let Err(reqwest_err) = result {
            assert!(
                is_network_error(&reqwest_err),
                "Should detect reqwest network errors"
            );
        } else {
            panic!("Expected a network error but request succeeded");
        }
    }

    #[test]
    fn device_flow_statuses_carry_static_context_but_keep_provider_text() {
        use but_error::{AnyhowContextExt as _, Code};
        let secret = "device_code=3584d274 user_code=WDJB-MJHT https://github.com/login/device";
        let rejected = Some((
            Code::GitHubDeviceFlowRejected,
            "GitHub rejected the device authorization request",
        ));
        let cases: [(&str, Option<(Code, &str)>); 9] = [
            ("authorization_pending", None),
            ("slow_down", None),
            (
                "expired_token",
                Some((
                    Code::GitHubDeviceCodeExpired,
                    "The GitHub device code has expired",
                )),
            ),
            (
                "access_denied",
                Some((
                    Code::GitHubDeviceAccessDenied,
                    "Authorization was denied on GitHub",
                )),
            ),
            ("incorrect_client_credentials", rejected),
            ("incorrect_device_code", rejected),
            ("unsupported_grant_type", rejected),
            ("device_flow_disabled", rejected),
            ("brand_new_status", rejected),
        ];
        for (status, expected) in cases {
            let body = format!(r#"{{"error":"{status}","error_description":"{secret}"}}"#);
            let err = parse_github_oauth_response::<serde_json::Value>(&body)
                .expect_err("an OAuth error status is a failure");
            match (err.custom_context(), expected) {
                (None, None) => {}
                (Some(ctx), Some((code, message))) => {
                    assert_eq!(ctx.code, code, "{status}");
                    assert_eq!(ctx.message.as_deref(), Some(message), "{status}");
                }
                (ctx, expected) => panic!("{status}: got {ctx:?}, expected {expected:?}"),
            }
            // `but_api::json::Error` serializes the context message when present, else the chain.
            let api_message = err
                .custom_context_or_error_chain()
                .message
                .expect("always a message");
            if expected.is_some() {
                assert!(
                    !api_message.contains(secret) && !api_message.contains(status),
                    "{status}: provider detail must not reach the API: {api_message}"
                );
            } else {
                assert!(
                    api_message.contains(status),
                    "{status}: pending statuses keep today's message"
                );
            }
            // Alternate display (Lite, CLI) keeps the provider status as the inner error.
            let display = format!("{err:#}");
            assert!(
                display.contains(&format!("GitHub returned an error: {status} (")),
                "{status}: {display}"
            );
        }
    }

    #[test]
    fn non_provider_parse_failures_are_not_contextualized() {
        use but_error::AnyhowContextExt as _;
        for body in ["not json", r#"{"unexpected":"shape"}"#] {
            let err = parse_github_oauth_response::<Verification>(body).expect_err("bad body");
            assert!(err.custom_context().is_none(), "{body:?}: {err:#}");
        }
    }
}
