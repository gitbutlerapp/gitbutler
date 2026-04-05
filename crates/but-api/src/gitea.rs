use anyhow::Result;
use but_api_macros::but_api;
use but_gitea::{AuthStatusResponse, AuthenticatedUser, json};
use but_secret::Sensitive;
use tracing::instrument;

// Fix for Issue #2904 - Gitea Support
/// Stores a Gitea Personal Access Token (PAT) for gitea.com.
///
/// Validates and stores the provided PAT, then retrieves and returns the authenticated
/// user information. The token is securely stored in the application's data directory.
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitea_pat(access_token: Sensitive<String>) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::store_pat(&access_token, &storage).await
}

/// Stores a Gitea Personal Access Token (PAT) for a self-hosted Gitea instance.
///
/// Validates and stores the provided PAT for a specific self-hosted Gitea host,
/// then retrieves and returns the authenticated user information.
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitea_selfhosted_pat(
    access_token: Sensitive<String>,
    host: String,
) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::store_selfhosted_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific Gitea account.
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
#[but_api(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gitea_user(
    account: but_gitea::GiteaAccountIdentifier,
) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::get_gitea_user(&account, &storage).await
}

/// Lists all Gitea accounts with stored credentials.
#[but_api]
#[instrument(err(Debug))]
pub fn list_known_gitea_accounts() -> Result<Vec<but_gitea::GiteaAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::list_known_gitea_accounts(&storage)
}

/// Validates stored Gitea credentials.
#[but_api]
#[instrument(err(Debug))]
pub async fn check_gitea_credentials(
    account: but_gitea::GiteaAccountIdentifier,
) -> Result<but_gitea::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::check_credentials(&account, &storage).await
}
