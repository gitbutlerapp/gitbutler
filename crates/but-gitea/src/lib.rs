//! Minimal Gitea account authentication and token-management support for GitButler.
//!
//! The crate is intentionally narrow: it validates a PAT against a Gitea-compatible
//! host, stores the authenticated account metadata, and lets callers rehydrate or
//! forget stored accounts later. Higher-level forge features are deliberately left
//! to follow-up work so this slice stays reviewable.

use anyhow::{Context as _, Result};
use but_secret::Sensitive;

mod client;
mod token;

pub use client::AuthenticatedUser as ClientAuthenticatedUser;
pub use token::GiteaAccountIdentifier;

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    /// The access token.
    ///
    /// This is returned only so the current desktop integration can continue to
    /// use the token immediately after storing it.
    pub access_token: Sensitive<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: String,
}

/// Validate a Gitea PAT for `host`, persist it securely, and return the resolved account.
pub async fn store_account(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_user_data(host, access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        username: user.username,
        name: user.name,
        email: user.email,
        host: host.to_owned(),
    })
}

/// Fetch the authenticated user from Gitea and persist the token under that account id.
async fn fetch_and_persist_user_data(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser> {
    let client = client::GiteaClient::new_with_host_override(access_token, host)
        .context("Failed to create Gitea client")?;
    let user = client
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gitea_access_token(
        &token::GiteaAccountIdentifier::new(&user.username, host),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

/// Delete the stored token and metadata for a single Gitea account.
pub fn forget_gitea_access_token(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    token::delete_gitea_access_token(account, storage).context("Failed to delete access token")
}

/// Resolve a stored Gitea account and return the current authenticated user profile, if any.
pub async fn get_gitea_user(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gitea_access_token(account, storage)? {
        let client = account
            .client(&access_token)
            .context("Failed to create Gitea client")?;
        let user = match client.get_authenticated().await {
            Ok(user) => user,
            Err(client_err) => {
                if let Some(reqwest_err) = client_err.downcast_ref::<reqwest::Error>()
                    && is_network_error(reqwest_err)
                {
                    return Err(client_err.context(but_error::Context::new_static(
                        but_error::Code::NetworkError,
                        "Unable to connect to Gitea.",
                    )));
                }
                return Err(client_err.context("Failed to get authenticated user"));
            }
        };
        Ok(Some(AuthenticatedUser {
            access_token,
            username: user.username,
            avatar_url: user.avatar_url,
            name: user.name,
            email: user.email,
            host: account.host.clone(),
        }))
    } else {
        Ok(None)
    }
}

fn is_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

/// List all Gitea accounts whose metadata is currently persisted locally.
pub fn list_known_gitea_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GiteaAccountIdentifier>> {
    token::list_known_gitea_accounts(storage).context("Failed to list known Gitea accounts")
}

/// Delete all stored Gitea accounts and their associated tokens.
pub fn clear_all_gitea_tokens(storage: &but_forge_storage::Controller) -> Result<()> {
    token::clear_all_gitea_accounts(storage).context("Failed to clear all Gitea tokens")
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// The access token used to authenticate the live request.
    pub access_token: Sensitive<String>,
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: String,
}

pub mod json {
    use serde::Serialize;

    use crate::{AuthStatusResponse, AuthenticatedUser};

    /// Serializable version of [`AuthStatusResponse`] with the token exposed as a string.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/index.ts"))]
    pub struct AuthStatusResponseSensitive {
        pub access_token: String,
        pub username: String,
        pub name: Option<String>,
        pub email: Option<String>,
        pub host: String,
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
            Self {
                access_token: access_token.0,
                username,
                name,
                email,
                host,
            }
        }
    }

    /// Serializable version of [`AuthenticatedUser`] with the token exposed as a string.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/index.ts"))]
    pub struct AuthenticatedUserSensitive {
        pub access_token: String,
        pub username: String,
        pub avatar_url: Option<String>,
        pub name: Option<String>,
        pub email: Option<String>,
        pub host: String,
    }

    impl From<AuthenticatedUser> for AuthenticatedUserSensitive {
        fn from(
            AuthenticatedUser {
                access_token,
                username,
                avatar_url,
                name,
                email,
                host,
            }: AuthenticatedUser,
        ) -> Self {
            Self {
                access_token: access_token.0,
                username,
                avatar_url,
                name,
                email,
                host,
            }
        }
    }
}
