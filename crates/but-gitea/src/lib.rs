pub mod client;
pub mod pr;
mod token;

pub use client::{AuthenticatedUser, GiteaClient, GiteaPrLabel, GiteaUser, PullRequest};
pub use token::GiteaAccountIdentifier;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/index.ts"))]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    pub access_token: but_secret::Sensitive<String>,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: String,
}

pub async fn store_pat(
    host: &str,
    access_token: &but_secret::Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<AuthStatusResponse> {
    let client = GiteaClient::new(access_token, host)?;
    let user = client.get_authenticated(access_token).await?;
    let account = GiteaAccountIdentifier::selfhosted(&user.login, host);
    token::persist_gt_access_token(&account, access_token, storage)?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        login: user.login,
        name: user.name,
        email: user.email,
        host: host.to_string(),
    })
}

pub async fn get_gt_user(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gt_access_token(account, storage)? {
        let client = account.client(&access_token)?;
        Ok(Some(client.get_authenticated(&access_token).await?))
    } else {
        Ok(None)
    }
}

pub async fn check_credentials(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<CredentialCheckResult> {
    if let Some(access_token) = token::get_gt_access_token(account, storage)? {
        let gt = account
            .client(&access_token)
            .map_err(|e| anyhow::anyhow!("Failed to create Gitea client: {e}"))?;
        match gt.get_authenticated(&access_token).await {
            Ok(_) => Ok(CredentialCheckResult::Valid),
            Err(_) => Ok(CredentialCheckResult::Invalid),
        }
    } else {
        Ok(CredentialCheckResult::NoCredentials)
    }
}

pub fn list_known_gitea_accounts(
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<Vec<GiteaAccountIdentifier>> {
    token::list_known_gitea_accounts(storage)
}

pub fn clear_all_gitea_tokens(storage: &but_forge_storage::Controller) -> anyhow::Result<()> {
    token::clear_all_gitea_tokens(storage)
}

pub fn forget_gt_access_token(
    account: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<()> {
    token::delete_gt_access_token(account, storage)
}

pub mod json {
    use super::*;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/index.ts"))]
    pub struct AuthStatusResponseSensitive {
        pub access_token: String,
        pub login: String,
        pub name: Option<String>,
        pub email: Option<String>,
        pub host: String,
    }

    impl From<AuthStatusResponse> for AuthStatusResponseSensitive {
        fn from(
            AuthStatusResponse {
                access_token,
                login,
                name,
                email,
                host,
            }: AuthStatusResponse,
        ) -> Self {
            AuthStatusResponseSensitive {
                access_token: access_token.0,
                login,
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
        pub login: String,
        pub avatar_url: Option<String>,
        pub name: Option<String>,
        pub email: Option<String>,
    }

    impl From<AuthenticatedUser> for AuthenticatedUserSensitive {
        fn from(
            AuthenticatedUser {
                access_token,
                login,
                avatar_url,
                name,
                email,
            }: AuthenticatedUser,
        ) -> Self {
            AuthenticatedUserSensitive {
                access_token: access_token.0,
                login,
                avatar_url,
                name,
                email,
            }
        }
    }
}
