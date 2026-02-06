use anyhow::{Context as _, Result};
use but_secret::Sensitive;

mod client;
pub mod pr;
pub use client::{
    CreatePullRequestParams, GiteaClient, GiteaLabel, GiteaUser, PullRequest, UpdatePullRequestParams,
};
mod token;
pub use token::GiteaAccountIdentifier;

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    /// The access token.
    /// This is only shared with the FrontEnd temporarily as we undergo the migration to having all API calls
    /// made to the forges from the Rustend.
    pub access_token: Sensitive<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: String,
}

/// Store a PAT access token for a Gitea instance and fetch the associated user data.
/// Gitea is always self-hosted, so a host URL is always required.
pub async fn store_pat(
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

/// Fetch the authenticated user data from Gitea and persist the access token.
async fn fetch_and_persist_user_data(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let client = client::GiteaClient::new(access_token, host).context("Failed to create Gitea client")?;
    let user = client
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gitea_access_token(
        &token::GiteaAccountIdentifier::selfhosted(&user.username, host),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

pub fn forget_gitea_access_token(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    token::delete_gitea_access_token(account, storage).context("Failed to delete access token")
}

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
            username: user.username,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
        }))
    } else {
        Ok(None)
    }
}

fn is_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

/// Check the validity of the stored credentials for the given Gitea account.
pub async fn check_credentials(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<CredentialCheckResult> {
    if let Some(access_token) = token::get_gitea_access_token(account, storage)? {
        let client = account
            .client(&access_token)
            .context("Failed to create Gitea client")?;
        match client.get_authenticated().await {
            Ok(_) => Ok(CredentialCheckResult::Valid),
            Err(_) => Ok(CredentialCheckResult::Invalid),
        }
    } else {
        Ok(CredentialCheckResult::NoCredentials)
    }
}

pub async fn list_known_gitea_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<token::GiteaAccountIdentifier>> {
    token::list_known_gitea_accounts(storage).context("Failed to list known Gitea accounts")
}

pub fn clear_all_gitea_tokens(storage: &but_forge_storage::Controller) -> Result<()> {
    token::clear_all_gitea_accounts(storage).context("Failed to clear all Gitea tokens")
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}
