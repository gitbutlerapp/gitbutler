use anyhow::{Context as _, Result};
use but_secret::Sensitive;

mod client;
pub mod mr;
mod project;
pub use client::{
    CreateMergeRequestParams, GitLabClient, GitLabLabel, GitLabUser, MergeMergeRequestParams, MergeRequest,
};
pub use project::GitLabProjectId;
mod token;
use serde::Serialize;
pub use token::GitlabAccountIdentifier;

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    /// The access token.
    /// This is only shared with the FrontEnd temporarily as we undergo the migration to having all API calls
    /// made to the forges from the Rustend.
    pub access_token: Sensitive<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: Option<String>,
}

/// Store a PAT access token and fetch the associated user data.
pub async fn store_pat(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_pat_user_data(access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        username: user.username,
        name: user.name,
        email: user.email,
        host: None,
    })
}

/// Fetch the authenticated user data from GitLab and persist the access token. (PAT)
async fn fetch_and_persist_pat_user_data(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gl = client::GitLabClient::new(access_token).context("Failed to create GitLab client")?;
    let user = gl
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gl_access_token(
        &token::GitlabAccountIdentifier::pat(&user.username),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

/// Store a self-hosted GitLab access token and fetch the associated user data.
pub async fn store_selfhosted_pat(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_selfhosted_user_data(host, access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        username: user.username,
        name: user.name,
        email: user.email,
        host: Some(host.to_owned()),
    })
}

/// Fetch the authenticated user data from GitLab and persist the access token. (Self-hosted)
async fn fetch_and_persist_selfhosted_user_data(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gl =
        client::GitLabClient::new_with_host_override(access_token, host).context("Failed to create GitLab client")?;
    let user = gl
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gl_access_token(
        &token::GitlabAccountIdentifier::selfhosted(&user.username, host),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

pub fn forget_gl_access_token(
    account: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    token::delete_gl_access_token(account, storage).context("Failed to delete access token")
}

pub async fn get_gl_user(
    account: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gl_access_token(account, storage)? {
        let gl = account
            .client(&access_token)
            .context("Failed to create GitLab client")?;
        let user = match gl.get_authenticated().await {
            Ok(user) => user,
            Err(client_err) => {
                println!("Error fetching authenticated user: {client_err:?}");
                // Check if this is a network error
                if let Some(reqwest_err) = client_err.downcast_ref::<reqwest::Error>()
                    && is_network_error(reqwest_err)
                {
                    return Err(client_err.context(but_error::Context::new_static(
                        but_error::Code::NetworkError,
                        "Unable to connect to GitLab.",
                    )));
                }
                return Err(client_err.context("Failed to get authenticated user"));
            }
        };
        Ok(Some(AuthenticatedUser {
            access_token,
            username: user.username,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
        }))
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

/// Check the validity of the stored credentials for the given GitLab account.
pub async fn check_credentials(
    account: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<CredentialCheckResult> {
    if let Some(access_token) = token::get_gl_access_token(account, storage)? {
        let gl = account
            .client(&access_token)
            .context("Failed to create GitLab client")?;
        match gl.get_authenticated().await {
            Ok(_) => Ok(CredentialCheckResult::Valid),
            Err(_) => Ok(CredentialCheckResult::Invalid),
        }
    } else {
        Ok(CredentialCheckResult::NoCredentials)
    }
}

pub async fn list_known_gitlab_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<token::GitlabAccountIdentifier>> {
    token::list_known_gitlab_accounts(storage).context("Failed to list known GitLab usernames")
}

pub fn clear_all_gitlab_tokens(storage: &but_forge_storage::Controller) -> Result<()> {
    token::clear_all_gitlab_accounts(storage).context("Failed to clear all GitLab tokens")
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub access_token: Sensitive<String>,
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// JSON serialization types for GitLab API responses.
///
/// This module contains serializable versions of GitLab authentication types
/// that expose sensitive data (like access tokens) as plain strings for API responses.
pub mod json {
    use crate::{AuthStatusResponse, AuthenticatedUser};
    use serde::Serialize;

    /// Serializable version of [`AuthStatusResponse`] with exposed access token.
    ///
    /// This struct is used for API responses where the access token needs to be
    /// sent as a plain string. Field names are converted to camelCase for JSON.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitlab/index.ts"))]
    pub struct AuthStatusResponseSensitive {
        /// The GitLab access token as a plain string (sensitive data).
        pub access_token: String,
        /// The GitLab username.
        pub username: String,
        /// The user's display name, if available.
        pub name: Option<String>,
        /// The user's email address, if available.
        pub email: Option<String>,
        /// The self-hosted GitLab host, if this is a self-hosted instance.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub host: Option<String>,
    }

    impl From<AuthStatusResponse> for AuthStatusResponseSensitive {
        fn from(
            AuthStatusResponse {
                access_token,
                username,
                name,
                email,
                host,
            }: AuthStatusResponse,
        ) -> Self {
            AuthStatusResponseSensitive {
                access_token: access_token.0,
                username,
                name,
                email,
                host,
            }
        }
    }

    /// Serializable version of [`AuthenticatedUser`] with exposed access token.
    ///
    /// This struct represents an authenticated GitLab user with their credentials
    /// exposed as plain strings for API responses. Field names are converted to camelCase for JSON.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitlab/index.ts"))]
    pub struct AuthenticatedUserSensitive {
        /// The GitLab access token as a plain string (sensitive data).
        pub access_token: String,
        /// The GitLab username.
        pub username: String,
        /// The URL to the user's avatar image, if available.
        pub avatar_url: Option<String>,
        /// The user's display name, if available.
        pub name: Option<String>,
        /// The user's email address, if available.
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
            assert!(is_network_error(&reqwest_err), "Should detect reqwest network errors");
        } else {
            panic!("Expected a network error but request succeeded");
        }
    }
}
