//! A thin wrapper of the Forgejo REST API, for authentication and resource access.
//!
//! Accounts are personal access tokens scoped to one instance; see
//! [`ForgejoAccountIdentifier`].

use anyhow::{Context as _, Result};
use but_secret::Sensitive;
use serde::Serialize;

pub mod checks;
mod client;
pub mod pr;
mod token;
pub use client::{
    CreatePullRequestParams, ForgejoClient, ForgejoCommitStatus, ForgejoLabel, ForgejoPermissions,
    ForgejoPullRequest, ForgejoRepo, ForgejoUser, HttpStatusError, MergeStyle,
    UpdatePullRequestParams,
};
pub use token::{ForgejoAccountIdentifier, normalize_host};

/// Who authenticated when a token was stored.
#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: String,
}

/// Validate a personal access token against the instance at `host`, then store it.
///
/// Fails without storing anything when the instance rejects the token.
pub async fn store_pat(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let host = normalize_host(host);
    let user = ForgejoClient::new(access_token, &host)
        .context("Failed to create Forgejo client")?
        .get_authenticated()
        .await
        .map_err(classify_pat_validation_error)
        .context("Failed to get authenticated user")?;
    let account_id = ForgejoAccountIdentifier::new(&user.username, &host);
    token::persist_access_token(&account_id, access_token, storage)
        .context("Failed to persist access token")?;
    cache_user_profile(&account_id, &user, storage);
    Ok(AuthStatusResponse {
        username: user.username,
        name: user.name,
        email: user.email,
        host,
    })
}

fn classify_pat_validation_error(err: anyhow::Error) -> anyhow::Error {
    let message = match err.downcast_ref::<HttpStatusError>().map(|e| e.status) {
        Some(reqwest::StatusCode::UNAUTHORIZED) => "Forgejo did not accept the token.",
        Some(reqwest::StatusCode::FORBIDDEN) => {
            "Forgejo refused access for the token. It needs the read:user scope."
        }
        Some(reqwest::StatusCode::NOT_FOUND) => {
            "No Forgejo API found at this address. Check the instance URL."
        }
        _ if err
            .downcast_ref::<reqwest::Error>()
            .is_some_and(is_network_error) =>
        {
            "Could not reach this address. Check the instance URL."
        }
        _ => return err,
    };
    // A `but_error::Context` is what the frontend shows the user.
    err.context(but_error::Context::new_static(
        but_error::Code::Validation,
        message,
    ))
}

/// Cache the user profile so it's available offline.
fn cache_user_profile(
    account: &ForgejoAccountIdentifier,
    user: &client::AuthenticatedUser,
    storage: &but_forge_storage::Controller,
) {
    let profile = but_forge_storage::settings::CachedProfile {
        avatar_url: user.avatar_url.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
    };
    let key = account.cache_key();
    if storage.cached_profile(&key).ok().flatten().as_ref() == Some(&profile) {
        return;
    }
    if let Err(err) = storage.set_cached_profile(&key, Some(profile)) {
        tracing::warn!(?account, "Failed to update cached Forgejo profile: {err}");
    }
}

pub fn forget_fj_access_token(
    account: &ForgejoAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    token::delete_access_token(account, storage).context("Failed to delete access token")
}

/// The profile of a stored account, or `None` when it has no stored token.
///
/// Serves the cached profile when the instance is unreachable, and clears it
/// when the instance rejects the token.
pub async fn get_fj_user(
    account: &ForgejoAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<AuthenticatedUser>> {
    let Some(access_token) = token::get_access_token(account, storage)? else {
        return Ok(None);
    };
    let client = account
        .client(&access_token)
        .context("Failed to create Forgejo client")?;
    let cache_key = account.cache_key();
    match client.get_authenticated().await {
        Ok(user) => {
            cache_user_profile(account, &user, storage);
            Ok(Some(AuthenticatedUser {
                username: user.username,
                host: account.host.clone(),
                avatar_url: user.avatar_url,
                name: user.name,
                email: user.email,
            }))
        }
        Err(err)
            if err
                .downcast_ref::<reqwest::Error>()
                .is_some_and(is_network_error) =>
        {
            if let Some(cached) = storage.cached_profile(&cache_key).ok().flatten() {
                return Ok(Some(AuthenticatedUser {
                    username: account.username.clone(),
                    host: account.host.clone(),
                    avatar_url: cached.avatar_url,
                    name: cached.name,
                    email: cached.email,
                }));
            }
            Err(err.context(but_error::Context::new_static(
                but_error::Code::NetworkError,
                "Unable to connect to Forgejo.",
            )))
        }
        Err(err) => {
            if err.downcast_ref::<HttpStatusError>().is_some_and(|e| {
                matches!(
                    e.status,
                    reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
                )
            }) && let Err(clear_err) = storage.set_cached_profile(&cache_key, None)
            {
                tracing::warn!("Failed to clear cached Forgejo profile: {clear_err}");
            }
            Err(err.context("Failed to get authenticated user"))
        }
    }
}

/// Check if an error is a network connectivity error.
///
/// This includes DNS resolution failures, connection timeouts, connection
/// refused, and connections dropped while the response body was being read.
/// A serde cause in the chain means the payload was malformed instead.
fn is_network_error(err: &reqwest::Error) -> bool {
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
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CredentialCheckResult);

/// Check the validity of the stored credentials for the given Forgejo account.
pub async fn check_credentials(
    account: &ForgejoAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<CredentialCheckResult> {
    let Some(access_token) = token::get_access_token(account, storage)? else {
        return Ok(CredentialCheckResult::NoCredentials);
    };
    let client = account
        .client(&access_token)
        .context("Failed to create Forgejo client")?;
    Ok(match client.get_authenticated().await {
        Ok(_) => CredentialCheckResult::Valid,
        Err(_) => CredentialCheckResult::Invalid,
    })
}

pub fn list_known_forgejo_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgejoAccountIdentifier>> {
    token::list_known_accounts(storage).context("Failed to list known Forgejo accounts")
}

pub fn clear_all_forgejo_tokens(storage: &but_forge_storage::Controller) -> Result<()> {
    token::clear_all_accounts(storage).context("Failed to clear all Forgejo tokens")
}

/// Fetch repository metadata (fork status, caller permissions) for `owner/repo`.
pub async fn fetch_repo(
    preferred_account: Option<&ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<ForgejoRepo> {
    ForgejoClient::from_storage(storage, preferred_account)?
        .fetch_repo(owner, repo)
        .await
        .context("Failed to fetch Forgejo repository")
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub host: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// JSON serialization types for Forgejo API responses.
///
/// Neither carries the access token: the frontend never talks to Forgejo directly.
pub mod json {
    use serde::Serialize;

    use crate::{AuthStatusResponse, AuthenticatedUser};

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ForgejoAuthStatusResponse {
        pub username: String,
        pub name: Option<String>,
        pub email: Option<String>,
        pub host: String,
    }

    impl From<AuthStatusResponse> for ForgejoAuthStatusResponse {
        fn from(
            AuthStatusResponse {
                username,
                name,
                email,
                host,
            }: AuthStatusResponse,
        ) -> Self {
            ForgejoAuthStatusResponse {
                username,
                name,
                email,
                host,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ForgejoAuthStatusResponse);

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ForgejoAuthenticatedUser {
        pub username: String,
        pub host: String,
        pub avatar_url: Option<String>,
        pub name: Option<String>,
        pub email: Option<String>,
    }

    impl From<AuthenticatedUser> for ForgejoAuthenticatedUser {
        fn from(
            AuthenticatedUser {
                username,
                host,
                avatar_url,
                name,
                email,
            }: AuthenticatedUser,
        ) -> Self {
            ForgejoAuthenticatedUser {
                username,
                host,
                avatar_url,
                name,
                email,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ForgejoAuthenticatedUser);
}
