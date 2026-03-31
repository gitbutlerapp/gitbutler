use anyhow::{Context as _, Result};
use but_secret::Sensitive;
use serde::Serialize;

mod client;
pub mod pr;
mod token;
pub use client::{
    CreatePullRequestParams, GiteaClient, GiteaLabel, GiteaUser, MergeMethod,
    MergePullRequestParams, PullRequest, SetPullRequestAutoMergeParams,
    SetPullRequestDraftStateParams, UpdatePullRequestParams,
};
pub use token::GiteaAccountIdentifier;

// Fix for Issue #2904 - Gitea Support
#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    pub access_token: Sensitive<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: Option<String>,
}

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

async fn fetch_and_persist_pat_user_data(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gitea = client::GiteaClient::new(access_token).context("Failed to create Gitea client")?;
    let user = gitea
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    token::persist_gitea_access_token(
        &token::GiteaAccountIdentifier::pat(&user.username),
        access_token,
        storage,
    )
    .context("Failed to persist access token")?;
    Ok(user)
}

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

async fn fetch_and_persist_selfhosted_user_data(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::AuthenticatedUser, anyhow::Error> {
    let gitea = client::GiteaClient::new_with_host_override(access_token, host)
        .context("Failed to create Gitea client")?;
    let user = gitea
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

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub access_token: Sensitive<String>,
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

pub async fn get_gitea_user(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gitea_access_token(account, storage)? {
        let gitea = account
            .client(&access_token)
            .context("Failed to create Gitea client")?;
        let user = match gitea.get_authenticated().await {
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
        }))
    } else {
        Ok(None)
    }
}

fn is_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

pub async fn check_credentials(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<CredentialCheckResult> {
    if let Some(access_token) = token::get_gitea_access_token(account, storage)? {
        let gitea = account
            .client(&access_token)
            .context("Failed to create Gitea client")?;
        match gitea.get_authenticated().await {
            Ok(_) => Ok(CredentialCheckResult::Valid),
            Err(_) => Ok(CredentialCheckResult::Invalid),
        }
    } else {
        Ok(CredentialCheckResult::NoCredentials)
    }
}

pub fn list_known_gitea_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<token::GiteaAccountIdentifier>> {
    token::list_known_gitea_accounts(storage).context("Failed to list known Gitea usernames")
}

pub fn clear_all_gitea_tokens(storage: &but_forge_storage::Controller) -> Result<()> {
    token::clear_all_gitea_accounts(storage).context("Failed to clear all Gitea tokens")
}

pub mod json {
    use serde::Serialize;

    use crate::{AuthStatusResponse, AuthenticatedUser};

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/index.ts"))]
    pub struct AuthStatusResponseSensitive {
        pub access_token: String,
        pub username: String,
        pub name: Option<String>,
        pub email: Option<String>,
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
