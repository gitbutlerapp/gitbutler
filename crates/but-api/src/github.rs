use anyhow::Result;
use but_api_macros::but_api;
use but_github::{AuthStatusResponse, AuthenticatedUser, Verification, json};
use but_secret::Sensitive;
use tracing::instrument;

/// Initiates the GitHub device OAuth flow.
///
/// This starts the OAuth device authorization flow, which allows users to authenticate
/// by visiting a URL and entering a code. Returns verification details including the
/// user code and verification URL.
///
/// # Returns
///
/// * `Ok(Verification)` - Contains the user code, device code, and verification URL
/// * `Err(_)` - If the OAuth initialization request fails
#[but_api]
#[instrument(err(Debug))]
pub async fn init_github_device_oauth() -> Result<Verification> {
    but_github::init_github_device_oauth().await
}

/// Checks the status of a GitHub device OAuth authorization.
///
/// Polls the GitHub API to check if the user has completed the device authorization flow.
/// If successful, stores the access token and returns the authenticated user information.
///
/// # Arguments
///
/// * `device_code` - The device code received from [`init_github_device_oauth`]
///
/// # Returns
///
/// * `Ok(AuthStatusResponse)` - User is authenticated, contains access token and user details
/// * `Err(_)` - If the authorization is pending, denied, or the request fails
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn check_github_auth_status(device_code: String) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::check_github_auth_status(device_code, &storage).await
}

/// Stores a GitHub Personal Access Token (PAT) for github.com.
///
/// Validates and stores the provided PAT, then retrieves and returns the authenticated
/// user information. The token is securely stored in the application's data directory.
///
/// # Arguments
///
/// * `access_token` - The GitHub PAT to store (wrapped in Sensitive to prevent logging)
///
/// # Returns
///
/// * `Ok(AuthStatusResponse)` - Token is valid, contains user details
/// * `Err(_)` - If the token is invalid or storage fails
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_github_pat(access_token: Sensitive<String>) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::store_pat(&access_token, &storage).await
}

/// Stores a GitHub Personal Access Token (PAT) for a GitHub Enterprise instance.
///
/// Validates and stores the provided PAT for a specific GitHub Enterprise host,
/// then retrieves and returns the authenticated user information.
///
/// # Arguments
///
/// * `access_token` - The GitHub Enterprise PAT to store
/// * `host` - The GitHub Enterprise hostname (e.g., "github.company.com")
///
/// # Returns
///
/// * `Ok(AuthStatusResponse)` - Token is valid, contains user details and host
/// * `Err(_)` - If the token is invalid, host is unreachable, or storage fails
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_github_enterprise_pat(access_token: Sensitive<String>, host: String) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::store_enterprise_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific GitHub account.
///
/// Deletes the access token associated with the specified GitHub account identifier.
/// This is used when users want to sign out or remove an account.
///
/// # Arguments
///
/// * `account` - Identifier for the GitHub account (github.com or enterprise)
///
/// # Returns
///
/// * `Ok(())` - Always succeeds, even if no token was found
#[but_api]
#[instrument(err(Debug))]
pub fn forget_github_account(account: but_github::GithubAccountIdentifier) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::forget_gh_access_token(&account, &storage).ok();
    Ok(())
}

/// Removes all stored GitHub credentials.
///
/// Deletes all stored GitHub access tokens for both github.com and enterprise instances.
/// This is a destructive operation that signs out all GitHub accounts.
///
/// # Returns
///
/// * `Ok(())` - All tokens successfully cleared
/// * `Err(_)` - If storage cleanup fails
#[but_api]
#[instrument(err(Debug))]
pub fn clear_all_github_tokens() -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::clear_all_github_tokens(&storage)
}

/// Retrieves the authenticated user information for a GitHub account.
///
/// Fetches the stored credentials and current user profile for the specified GitHub account.
/// Returns `None` if no credentials are stored for the account.
///
/// # Arguments
///
/// * `account` - Identifier for the GitHub account to query
///
/// # Returns
///
/// * `Ok(Some(AuthenticatedUser))` - User information with access token
/// * `Ok(None)` - No credentials stored for this account
/// * `Err(_)` - If the API request fails or credentials are invalid
#[but_api(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gh_user(account: but_github::GithubAccountIdentifier) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::get_gh_user(&account, &storage).await
}

/// Lists all GitHub accounts with stored credentials.
///
/// Returns identifiers for all GitHub accounts (github.com and enterprise) that have
/// stored access tokens in the application.
///
/// # Returns
///
/// * `Ok(Vec<GithubAccountIdentifier>)` - List of all known accounts
/// * `Err(_)` - If storage access fails
#[but_api]
#[instrument(err(Debug))]
pub async fn list_known_github_accounts() -> Result<Vec<but_github::GithubAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::list_known_github_accounts(&storage).await
}

/// Validates stored GitHub credentials.
///
/// Checks if the stored credentials for the specified account are still valid by
/// making a test API request. This helps detect expired or revoked tokens.
///
/// # Arguments
///
/// * `account` - Identifier for the GitHub account to validate
///
/// # Returns
///
/// * `Ok(CredentialCheckResult)` - Result indicating if credentials are valid or not
/// * `Err(_)` - If the validation request fails
#[but_api]
#[instrument(err(Debug))]
pub async fn check_github_credentials(
    account: but_github::GithubAccountIdentifier,
) -> Result<but_github::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_github::check_credentials(&account, &storage).await
}
