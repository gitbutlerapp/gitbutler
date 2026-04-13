use anyhow::Result;
use but_api_macros::but_api;
use but_gitea::{AuthStatusResponse, AuthenticatedUser, json};
use but_secret::Sensitive;
use tracing::instrument;

/// Stores a Gitea Personal Access Token (PAT).
///
/// Validates and stores the provided PAT for a specific Gitea host,
/// then retrieves and returns the authenticated user information.
///
/// # Arguments
///
/// * `access_token` - The Gitea PAT to store
/// * `host` - The Gitea hostname (e.g., "gitea.com" or "gitea.company.com")
///
/// # Returns
///
/// * `Ok(AuthStatusResponse)` - Token is valid, contains user details and host
/// * `Err(_)` - If the token is invalid, host is unreachable, or storage fails
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitea_pat(
    access_token: Sensitive<String>,
    host: String,
) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::store_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific Gitea account.
///
/// Deletes the access token associated with the specified Gitea account identifier.
/// This is used when users want to sign out or remove an account.
///
/// # Arguments
///
/// * `account` - Identifier for the Gitea account
///
/// # Returns
///
/// * `Ok(())` - Always succeeds, even if no token was found
#[but_api]
#[instrument(err(Debug))]
pub fn forget_gitea_account(account: but_gitea::GiteaAccountIdentifier) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::forget_gt_access_token(&account, &storage).ok();
    Ok(())
}

/// Retrieves the authenticated user information for a Gitea account.
///
/// Fetches the stored credentials and current user profile for the specified Gitea account.
/// Returns `None` if no credentials are stored for the account.
///
/// # Arguments
///
/// * `account` - Identifier for the Gitea account to query
///
/// # Returns
///
/// * `Ok(Some(AuthenticatedUser))` - User information with access token
/// * `Ok(None)` - No credentials stored for this account
/// * `Err(_)` - If the API request fails or credentials are invalid
#[but_api(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gitea_user(
    account: but_gitea::GiteaAccountIdentifier,
) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::get_gt_user(&account, &storage).await
}

/// Lists all Gitea accounts with stored credentials.
///
/// Returns identifiers for all Gitea accounts that have stored access tokens in the application.
///
/// # Returns
///
/// * `Ok(Vec<GiteaAccountIdentifier>)` - List of all known accounts
/// * `Err(_)` - If storage access fails
#[but_api]
#[instrument(err(Debug))]
pub fn list_known_gitea_accounts() -> Result<Vec<but_gitea::GiteaAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::list_known_gitea_accounts(&storage)
}

/// Removes all stored Gitea credentials.
///
/// Deletes all stored Gitea access tokens.
/// This is a destructive operation that signs out all Gitea accounts.
///
/// # Returns
///
/// * `Ok(())` - All tokens successfully cleared
/// * `Err(_)` - If storage cleanup fails
#[but_api]
#[instrument(err(Debug))]
pub fn clear_all_gitea_tokens() -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::clear_all_gitea_tokens(&storage)
}

/// Validates stored Gitea credentials.
///
/// Checks if the stored credentials for the specified account are still valid by
/// making a test API request. This helps detect expired or revoked tokens.
///
/// # Arguments
///
/// * `account` - Identifier for the Gitea account to validate
///
/// # Returns
///
/// * `Ok(CredentialCheckResult)` - Result indicating if credentials are valid or not
/// * `Err(_)` - If the validation request fails
#[but_api]
#[instrument(err(Debug))]
pub async fn check_gitea_credentials(
    account: but_gitea::GiteaAccountIdentifier,
) -> Result<but_gitea::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitea::check_credentials(&account, &storage).await
}
