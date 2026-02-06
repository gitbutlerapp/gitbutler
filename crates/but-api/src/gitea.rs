use anyhow::Result;
use but_api_macros::but_api;
use but_gitea::{AuthStatusResponse, AuthenticatedUser};
use but_secret::Sensitive;
use tracing::instrument;

/// JSON serialization types for Gitea API responses.
pub mod json {
    use but_gitea::{AuthStatusResponse, AuthenticatedUser};
    use serde::Serialize;

    /// Serializable version of [`AuthStatusResponse`] with exposed access token.
    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AuthStatusResponseSensitive {
        /// The Gitea access token as a plain string (sensitive data).
        pub access_token: String,
        /// The Gitea username/login.
        pub username: String,
        /// The user's display name, if available.
        pub name: Option<String>,
        /// The user's email address, if available.
        pub email: Option<String>,
        /// The Gitea instance host.
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
            AuthStatusResponseSensitive {
                access_token: access_token.0,
                username,
                name,
                email,
                host,
            }
        }
    }

    /// Serializable version of [`AuthenticatedUser`].
    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AuthenticatedUserSensitive {
        /// The Gitea username/login.
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
                username,
                avatar_url,
                name,
                email,
            }: AuthenticatedUser,
        ) -> Self {
            AuthenticatedUserSensitive {
                username,
                avatar_url,
                name,
                email,
            }
        }
    }
}

/// Stores a Gitea Personal Access Token (PAT) for a Gitea instance.
///
/// Validates and stores the provided PAT for a specific Gitea host,
/// then retrieves and returns the authenticated user information.
///
/// # Arguments
///
/// * `access_token` - The Gitea PAT to store
/// * `host` - The Gitea instance URL (e.g., "https://gitea.example.com" or "https://codeberg.org")
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitea_pat(access_token: Sensitive<String>, host: String) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::store_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific Gitea account.
///
/// # Arguments
///
/// * `account` - Identifier for the Gitea account
#[but_api]
#[instrument(err(Debug))]
pub fn forget_gitea_account(account: but_gitea::GiteaAccountIdentifier) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::forget_gitea_access_token(&account, &storage).ok();
    Ok(())
}

/// Removes all stored Gitea credentials.
#[but_api]
#[instrument(err(Debug))]
pub fn clear_all_gitea_tokens() -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::clear_all_gitea_tokens(&storage)
}

/// Retrieves the authenticated user information for a Gitea account.
///
/// Returns `None` if no credentials are stored for the account.
///
/// # Arguments
///
/// * `account` - Identifier for the Gitea account to query
#[but_api(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gitea_user(account: but_gitea::GiteaAccountIdentifier) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::get_gitea_user(&account, &storage).await
}

/// Lists all Gitea accounts with stored credentials.
#[but_api]
#[instrument(err(Debug))]
pub async fn list_known_gitea_accounts() -> Result<Vec<but_gitea::GiteaAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::list_known_gitea_accounts(&storage).await
}

/// Validates stored Gitea credentials.
///
/// # Arguments
///
/// * `account` - Identifier for the Gitea account to validate
#[instrument(err(Debug))]
pub async fn check_gitea_credentials(
    account: but_gitea::GiteaAccountIdentifier,
) -> Result<but_gitea::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::check_credentials(&account, &storage).await
}
