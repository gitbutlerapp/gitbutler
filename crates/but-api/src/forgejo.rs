//! forgejo-fork: Forgejo account management, mirroring `gitlab.rs`.

use anyhow::Result;
use but_api_macros::but_api;
use but_forgejo::{AuthStatusResponse, AuthenticatedUser, json};
use but_secret::Sensitive;
use tracing::instrument;

/// Stores a Forgejo Personal Access Token (PAT) for the instance at `host`.
///
/// The token is validated against the instance before it is stored, and the
/// authenticated user is returned.
///
/// # Arguments
///
/// * `access_token` - The Forgejo PAT (needs `read:user`, `write:repository`, `write:issue`)
/// * `host` - The instance address, e.g. `codeberg.org` or `https://git.example.com`
///
/// # Returns
///
/// * `Ok(_)` - The token is valid and stored
/// * `Err(_)` - If the token is rejected, the host is unreachable, or storage fails
#[but_api(napi, json::ForgejoAuthStatusResponse, invalidates = [ForgeAccounts, ForgeLogin])]
#[instrument(err(Debug))]
pub async fn store_forgejo_pat(
    access_token: Sensitive<String>,
    host: String,
) -> Result<AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forgejo::store_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific Forgejo account.
///
/// # Returns
///
/// * `Ok(())` - Always succeeds, even if no token was found
#[but_api(napi, invalidates = [ForgeAccounts, ForgeLogin])]
#[instrument(err(Debug))]
pub fn forget_forgejo_account(account: but_forgejo::ForgejoAccountIdentifier) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forgejo::forget_fj_access_token(&account, &storage).ok();
    Ok(())
}

/// Removes all stored Forgejo credentials.
#[but_api]
#[instrument(err(Debug))]
pub fn clear_all_forgejo_tokens() -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forgejo::clear_all_forgejo_tokens(&storage)
}

/// Retrieves the profile of a stored Forgejo account.
///
/// # Returns
///
/// * `Ok(Some(_))` - The user's profile (cached when the instance is unreachable)
/// * `Ok(None)` - No credentials stored for this account
/// * `Err(_)` - If the instance rejects the token or the request fails
#[but_api(napi, json::ForgejoAuthenticatedUser)]
#[instrument(err(Debug))]
pub async fn get_fj_user(
    account: but_forgejo::ForgejoAccountIdentifier,
) -> Result<Option<AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forgejo::get_fj_user(&account, &storage).await
}

/// Lists all Forgejo accounts with stored credentials.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_known_forgejo_accounts() -> Result<Vec<but_forgejo::ForgejoAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forgejo::list_known_forgejo_accounts(&storage)
}

/// Validates the stored credentials of a Forgejo account against its instance.
#[but_api]
#[instrument(err(Debug))]
pub async fn check_forgejo_credentials(
    account: but_forgejo::ForgejoAccountIdentifier,
) -> Result<but_forgejo::CredentialCheckResult> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forgejo::check_credentials(&account, &storage).await
}
