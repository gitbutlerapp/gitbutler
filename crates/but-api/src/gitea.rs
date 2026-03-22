use anyhow::Result;
use but_api_macros::but_api;
use but_gitea::{AuthStatusResponse, AuthenticatedUser, json};
use but_secret::Sensitive;
use tracing::instrument;

/// Stores a Gitea access token for a given instance host.
///
/// Validates and stores the provided access token, then retrieves and returns the authenticated
/// user information for the configured Gitea-compatible host.
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitea_account(
    access_token: Sensitive<String>,
    host: String,
) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::store_account(&host, &access_token, &storage).await
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
