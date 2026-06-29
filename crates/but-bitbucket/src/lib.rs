use anyhow::{Context as _, Result};
use but_secret::Sensitive;

mod client;
pub mod pr;
mod repo;
pub use client::{
    BitbucketBuildStatus, BitbucketClient, BitbucketPullRequest, BitbucketRepo, BitbucketUser,
    CreatePullRequestParams, HttpStatusError, MergePullRequestParams, MergeStrategy,
    SetPullRequestDraftStateParams, UpdatePullRequestParams,
};
pub use repo::fetch_repo;
mod token;
use serde::Serialize;
pub use token::BitbucketAccountIdentifier;

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    /// The access token.
    /// This is only shared with the FrontEnd temporarily as we undergo the migration to having all API calls
    /// made to the forges from the Rustend.
    pub access_token: Sensitive<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Store an Atlassian API token and fetch the associated user data.
///
/// `email` is the Atlassian account email, used as the HTTP Basic username when
/// authenticating against Bitbucket Cloud.
pub async fn store_api_token(
    email: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_user_data(email, access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        username: user.username,
        name: user.name,
        email: Some(email.to_owned()),
    })
}

/// Cache the user profile so it's available offline.
fn cache_user_profile(
    account: &BitbucketAccountIdentifier,
    user: &client::AuthenticatedUser,
    email: &str,
    storage: &but_forge_storage::Controller,
) {
    let profile = but_forge_storage::settings::CachedProfile {
        avatar_url: user.avatar_url.clone(),
        name: user.name.clone(),
        email: Some(email.to_owned()),
    };
    let key = account.cache_key();
    let existing = storage.cached_profile(&key).ok().flatten();
    if existing.as_ref() == Some(&profile) {
        return;
    }
    if let Err(err) = storage.set_cached_profile(&key, Some(profile)) {
        tracing::warn!(?account, "Failed to update cached Bitbucket profile: {err}");
    }
}

/// Fetch the authenticated user data from Bitbucket and persist the access token.
async fn fetch_and_persist_user_data(
    email: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let bb = client::BitbucketClient::new(email, access_token)
        .context("Failed to create Bitbucket client")?;
    let user = bb
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    let account_id = token::BitbucketAccountIdentifier::apitoken(email);
    token::persist_bb_access_token(&account_id, access_token, storage)
        .context("Failed to persist access token")?;
    cache_user_profile(&account_id, &user, email, storage);
    Ok(user)
}

pub fn forget_bb_access_token(
    account: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    token::delete_bb_access_token(account, storage).context("Failed to delete access token")
}

pub async fn get_bb_user(
    account: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_bb_access_token(account, storage)? {
        let bb = account
            .client(&access_token)
            .context("Failed to create Bitbucket client")?;
        match bb.get_authenticated().await {
            Ok(user) => {
                cache_user_profile(account, &user, account.email(), storage);
                Ok(Some(AuthenticatedUser {
                    access_token,
                    username: user.username,
                    name: user.name,
                    email: Some(account.email().to_owned()),
                    avatar_url: user.avatar_url,
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
                                // Offline: no fetched handle available, fall back to the email.
                                username: account.email().to_owned(),
                                avatar_url: cached.avatar_url,
                                name: cached.name,
                                email: cached.email,
                            }));
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::warn!("Failed to read cached Bitbucket profile: {err}");
                        }
                    }
                    return Err(client_err.context(but_error::Context::new_static(
                        but_error::Code::NetworkError,
                        "Unable to connect to Bitbucket.",
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
                    tracing::warn!("Failed to clear cached Bitbucket profile: {err}");
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
/// This includes DNS resolution failures, connection timeouts, connection refused, etc.
fn is_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

/// Stable 64-bit hash (FNV-1a) for synthesizing numeric ids from the string
/// identifiers Bitbucket exposes, which have no native numeric id.
pub fn stable_id_hash(input: &str) -> i64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    hash as i64
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

/// Check the validity of the stored credentials for the given Bitbucket account.
pub async fn check_credentials(
    account: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<CredentialCheckResult> {
    if let Some(access_token) = token::get_bb_access_token(account, storage)? {
        let bb = account
            .client(&access_token)
            .context("Failed to create Bitbucket client")?;
        match bb.get_authenticated().await {
            Ok(_) => Ok(CredentialCheckResult::Valid),
            Err(_) => Ok(CredentialCheckResult::Invalid),
        }
    } else {
        Ok(CredentialCheckResult::NoCredentials)
    }
}

pub fn list_known_bitbucket_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<token::BitbucketAccountIdentifier>> {
    token::list_known_bitbucket_accounts(storage).context("Failed to list known Bitbucket accounts")
}

pub fn clear_all_bitbucket_tokens(storage: &but_forge_storage::Controller) -> Result<()> {
    token::clear_all_bitbucket_accounts(storage).context("Failed to clear all Bitbucket tokens")
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub access_token: Sensitive<String>,
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// JSON serialization types for Bitbucket API responses.
///
/// This module contains serializable versions of Bitbucket authentication types
/// that expose sensitive data (like access tokens) as plain strings for API responses.
pub mod json {
    use serde::Serialize;

    use crate::{AuthStatusResponse, AuthenticatedUser};

    /// Serializable version of [`AuthStatusResponse`] with exposed access token.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[cfg_attr(
        feature = "export-schema",
        schemars(rename = "BitbucketAuthStatusResponseSensitive")
    )]
    #[serde(rename_all = "camelCase")]
    pub struct AuthStatusResponseSensitive {
        /// The Bitbucket access token as a plain string (sensitive data).
        pub access_token: String,
        /// The Bitbucket username.
        pub username: String,
        /// The user's display name, if available.
        pub name: Option<String>,
        /// The Atlassian account email used for authentication.
        pub email: Option<String>,
    }

    impl From<AuthStatusResponse> for AuthStatusResponseSensitive {
        fn from(
            AuthStatusResponse {
                access_token,
                username,
                name,
                email,
            }: AuthStatusResponse,
        ) -> Self {
            AuthStatusResponseSensitive {
                access_token: access_token.0,
                username,
                name,
                email,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(AuthStatusResponseSensitive);

    /// Serializable version of [`AuthenticatedUser`] with exposed access token.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[cfg_attr(
        feature = "export-schema",
        schemars(rename = "BitbucketAuthenticatedUserSensitive")
    )]
    #[serde(rename_all = "camelCase")]
    pub struct AuthenticatedUserSensitive {
        /// The Bitbucket access token as a plain string (sensitive data).
        pub access_token: String,
        /// The Bitbucket username.
        pub username: String,
        /// The URL to the user's avatar image, if available.
        pub avatar_url: Option<String>,
        /// The user's display name, if available.
        pub name: Option<String>,
        /// The Atlassian account email used for authentication.
        pub email: Option<String>,
    }

    impl From<AuthenticatedUser> for AuthenticatedUserSensitive {
        fn from(
            AuthenticatedUser {
                access_token,
                username,
                avatar_url,
                name,
                email,
            }: AuthenticatedUser,
        ) -> Self {
            AuthenticatedUserSensitive {
                access_token: access_token.0,
                username,
                avatar_url,
                name,
                email,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(AuthenticatedUserSensitive);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_network_error_with_connection_error() {
        // Loopback port 1 is closed, so the connection is refused immediately
        // without touching the external network — deterministic and fast.
        let client = reqwest::blocking::Client::new();
        let err = client
            .get("http://127.0.0.1:1")
            .send()
            .expect_err("connection to a closed port should fail");
        assert!(
            is_network_error(&err),
            "connection refused should be classified as a network error"
        );
    }

    #[test]
    fn test_is_network_error_with_timeout() {
        use std::io::Read as _;
        use std::net::TcpListener;

        // A loopback listener that accepts but never replies forces a read
        // timeout - deterministic and without touching the external network.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 64];
                let _ = stream.read(&mut buf);
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(50))
            .build()
            .unwrap();
        let err = client
            .get(format!("http://{addr}/"))
            .send()
            .expect_err("request to a non-responding server should time out");
        assert!(
            is_network_error(&err),
            "a timeout should be classified as a network error"
        );

        let _ = server.join();
    }
}
