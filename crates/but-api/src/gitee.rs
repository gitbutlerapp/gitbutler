use anyhow::Result;
use but_api_macros::but_api;
use but_gitee::json;
use but_secret::Sensitive;
use tracing::instrument;

/// Stores a Gitee Personal Access Token (PAT) for gitee.com.
///
/// Validates and stores the provided PAT, then retrieves and returns the authenticated
/// user information. The token is securely stored in the system keychain.
#[but_api(napi, json::GiteeAuthStatusResponse, invalidates = [ForgeAccounts, ForgeLogin])]
#[instrument(err(Debug))]
pub async fn store_gitee_pat(
    access_token: Sensitive<String>,
) -> Result<but_gitee::AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitee::store_pat(&access_token, &storage).await
}

/// Stores a Gitee Personal Access Token (PAT) for a self-hosted (enterprise) Gitee instance.
#[but_api(napi, json::GiteeAuthStatusResponse, invalidates = [ForgeAccounts, ForgeLogin])]
#[instrument(err(Debug))]
pub async fn store_gitee_selfhosted_pat(
    access_token: Sensitive<String>,
    host: String,
) -> Result<but_gitee::AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitee::store_selfhosted_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific Gitee account.
#[but_api(napi, invalidates = [ForgeAccounts, ForgeLogin])]
#[instrument(err(Debug))]
pub fn forget_gitee_account(account: but_gitee::GiteeAccountIdentifier) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitee::forget_gitee_access_token(&account, &storage).ok();
    Ok(())
}

/// Retrieves the authenticated user information for a Gitee account.
#[but_api(napi, json::GiteeAuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gitee_user(
    account: but_gitee::GiteeAccountIdentifier,
) -> Result<Option<but_gitee::AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitee::get_gitee_user(&account, &storage).await
}

/// Lists all Gitee accounts with stored credentials.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_known_gitee_accounts() -> Result<Vec<but_gitee::GiteeAccountIdentifier>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_gitee::list_known_gitee_accounts(&storage)
}
