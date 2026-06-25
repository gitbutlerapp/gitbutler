use anyhow::Result;
use but_api_macros::but_api;
use but_bitbucket::{AuthStatusResponse, AuthenticatedUser, json};
use but_secret::Sensitive;
use tracing::instrument;

/// Stores an Atlassian API token for Bitbucket Cloud.
///
/// Bitbucket Cloud authenticates over HTTP Basic where the username is the
/// Atlassian account email and the password is the API token (with scopes).
/// Validates and stores the provided token, then returns the authenticated user.
///
/// # Arguments
///
/// * `email` - The Atlassian account email (HTTP Basic username)
/// * `access_token` - The Bitbucket API token to store (wrapped in Sensitive)
///
/// # Returns
///
/// * `Ok(AuthStatusResponse)` - Token is valid, contains user details
/// * `Err(_)` - If the token is invalid or storage fails
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_bitbucket_api_token(
    email: String,
    access_token: Sensitive<String>,
) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_bitbucket::store_api_token(&email, &access_token, &storage).await
}

/// Removes stored credentials for a specific Bitbucket account.
///
/// # Arguments
///
/// * `account` - Identifier for the Bitbucket account
///
/// # Returns
///
/// * `Ok(())` - Always succeeds, even if no token was found
#[but_api]
#[instrument(err(Debug))]
pub fn forget_bitbucket_account(account: but_bitbucket::BitbucketAccountIdentifier) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_bitbucket::forget_bb_access_token(&account, &storage).ok();
    Ok(())
}

/// Removes all stored Bitbucket credentials.
///
/// # Returns
///
/// * `Ok(())` - All tokens successfully cleared
/// * `Err(_)` - If storage cleanup fails
#[but_api]
#[instrument(err(Debug))]
pub fn clear_all_bitbucket_tokens() -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_bitbucket::clear_all_bitbucket_tokens(&storage)
}

/// Retrieves the authenticated user information for a Bitbucket account.
///
/// # Arguments
///
/// * `account` - Identifier for the Bitbucket account to query
///
/// # Returns
///
/// * `Ok(Some(AuthenticatedUser))` - User information
/// * `Ok(None)` - No credentials stored for this account
/// * `Err(_)` - If the API request fails or credentials are invalid
#[but_api(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_bb_user(
    account: but_bitbucket::BitbucketAccountIdentifier,
) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_bitbucket::get_bb_user(&account, &storage).await
}

/// Lists all Bitbucket accounts with stored credentials.
///
/// # Returns
///
/// * `Ok(Vec<BitbucketAccountIdentifier>)` - List of all known accounts
/// * `Err(_)` - If storage access fails
#[but_api]
#[instrument(err(Debug))]
pub fn list_known_bitbucket_accounts() -> Result<Vec<but_bitbucket::BitbucketAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_bitbucket::list_known_bitbucket_accounts(&storage)
}

/// Validates stored Bitbucket credentials.
///
/// # Arguments
///
/// * `account` - Identifier for the Bitbucket account to validate
///
/// # Returns
///
/// * `Ok(CredentialCheckResult)` - Result indicating if credentials are valid
/// * `Err(_)` - If the validation request fails
#[but_api]
#[instrument(err(Debug))]
pub async fn check_bitbucket_credentials(
    account: but_bitbucket::BitbucketAccountIdentifier,
) -> Result<but_bitbucket::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_bitbucket::check_credentials(&account, &storage).await
}
