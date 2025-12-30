use anyhow::{Context as _, Result};
use but_secret::Sensitive;

mod client;
pub use client::{CreatePullRequestParams, GiteaUser, PullRequest};
mod token;
pub use token::GiteaAccountIdentifier;
pub mod pr;

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    pub access_token: Sensitive<String>,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: String,
}

/// Store a Personal Access Token and fetch the associated user data.
pub async fn store_pat(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_user_data(host, access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        login: user.login,
        name: user.full_name,
        email: Some(user.email).filter(|e| !e.is_empty()),
        host: host.to_owned(),
    })
}

async fn fetch_and_persist_user_data(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::GiteaUser, anyhow::Error> {
    let client =
        client::GiteaClient::new(host, access_token).context("Failed to create Gitea client")?;
    let user = client
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gitea_access_token(
        &token::GiteaAccountIdentifier::new(&user.login, host),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}
