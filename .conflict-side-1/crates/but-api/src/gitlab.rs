use anyhow::Result;
use but_api_macros::but_api;
use but_gitlab::{AuthStatusResponse, AuthenticatedUser, json};
use but_secret::Sensitive;
use tracing::instrument;

/// Stores a GitLab Personal Access Token (PAT) for gitlab.com.
///
/// Validates and stores the provided PAT, then retrieves and returns the authenticated
/// user information. The token is securely stored in the application's data directory.
///
/// # Arguments
///
/// * `access_token` - The GitLab PAT to store (wrapped in Sensitive to prevent logging)
///
/// # Returns
///
/// * `Ok(AuthStatusResponse)` - Token is valid, contains user details
/// * `Err(_)` - If the token is invalid or storage fails
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitlab_pat(access_token: Sensitive<String>) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitlab::store_pat(&access_token, &storage).await
}

/// Stores a GitLab Personal Access Token (PAT) for a self-hosted GitLab instance.
///
/// Validates and stores the provided PAT for a specific self-hosted GitLab host,
/// then retrieves and returns the authenticated user information.
///
/// # Arguments
///
/// * `access_token` - The self-hosted GitLab PAT to store
/// * `host` - The self-hosted GitLab hostname (e.g., "gitlab.company.com")
///
/// # Returns
///
/// * `Ok(AuthStatusResponse)` - Token is valid, contains user details and host
/// * `Err(_)` - If the token is invalid, host is unreachable, or storage fails
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitlab_selfhosted_pat(
    access_token: Sensitive<String>,
    host: String,
) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitlab::store_selfhosted_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific GitLab account.
///
/// Deletes the access token associated with the specified GitLab account identifier.
/// This is used when users want to sign out or remove an account.
///
/// # Arguments
///
/// * `account` - Identifier for the GitLab account (gitlab.com or self-hosted)
///
/// # Returns
///
/// * `Ok(())` - Always succeeds, even if no token was found
#[but_api]
#[instrument(err(Debug))]
pub fn forget_gitlab_account(account: but_gitlab::GitlabAccountIdentifier) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitlab::forget_gl_access_token(&account, &storage).ok();
    Ok(())
}

/// Removes all stored GitLab credentials.
///
/// Deletes all stored GitLab access tokens for both gitlab.com and self-hosted instances.
/// This is a destructive operation that signs out all GitLab accounts.
///
/// # Returns
///
/// * `Ok(())` - All tokens successfully cleared
/// * `Err(_)` - If storage cleanup fails
#[but_api]
#[instrument(err(Debug))]
pub fn clear_all_gitlab_tokens() -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitlab::clear_all_gitlab_tokens(&storage)
}

/// Retrieves the authenticated user information for a GitLab account.
///
/// Fetches the stored credentials and current user profile for the specified GitLab account.
/// Returns `None` if no credentials are stored for the account.
///
/// # Arguments
///
/// * `account` - Identifier for the GitLab account to query
///
/// # Returns
///
/// * `Ok(Some(AuthenticatedUser))` - User information
/// * `Ok(None)` - No credentials stored for this account
/// * `Err(_)` - If the API request fails or credentials are invalid
#[but_api(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gl_user(
    account: but_gitlab::GitlabAccountIdentifier,
) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitlab::get_gl_user(&account, &storage).await
}

/// Lists all GitLab accounts with stored credentials.
///
/// Returns identifiers for all GitLab accounts (gitlab.com and self-hosted) that have
/// stored access tokens in the application.
///
/// # Returns
///
/// * `Ok(Vec<GitlabAccountIdentifier>)` - List of all known accounts
/// * `Err(_)` - If storage access fails
#[but_api]
#[instrument(err(Debug))]
pub fn list_known_gitlab_accounts() -> Result<Vec<but_gitlab::GitlabAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitlab::list_known_gitlab_accounts(&storage)
}

/// Validates stored GitLab credentials.
///
/// Checks if the stored credentials for the specified account are still valid by
/// making a test API request. This helps detect expired or revoked tokens.
///
/// # Arguments
///
/// * `account` - Identifier for the GitLab account to validate
///
/// # Returns
///
/// * `Ok(CredentialCheckResult)` - Result indicating if credentials are valid or not
/// * `Err(_)` - If the validation request fails
#[but_api]
#[instrument(err(Debug))]
pub async fn check_gitlab_credentials(
    account: but_gitlab::GitlabAccountIdentifier,
) -> Result<but_gitlab::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitlab::check_credentials(&account, &storage).await
}
