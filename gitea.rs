use anyhow::Result;
use but_api_macros::but_api;
use but_secret::Sensitive;
use tracing::instrument;

/// Stores a Gitea Personal Access Token (PAT).
/// 
/// Since Gitea follows a similar authentication structure to GitHub Enterprise,
/// we are leveraging the existing enterprise storage logic to maintain consistency.
#[but_api(json::AuthStatusResponseSensitive)]
#[instrument(err(Debug))]
pub async fn store_gitea_pat(
    access_token: Sensitive<String>,
    host: String,
) -> Result<but_github::AuthStatusResponse> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    // Gitea API is highly compatible with GitHub Enterprise PAT logic
    but_github::store_enterprise_pat(&host, &access_token, &storage).await
}

/// Removes stored credentials for a specific Gitea instance.
#[but_api]
#[instrument(err(Debug))]
pub fn forget_gitea_account(host: String) -> Result<()> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    let account = but_github::GithubAccountIdentifier::Enterprise { host };
    but_github::forget_gh_access_token(&account, &storage).ok();
    Ok(())
}

/// Retrieves authenticated user info for a Gitea account.
#[but_api(json::AuthenticatedUserSensitive)]
#[instrument(err(Debug))]
pub async fn get_gitea_user(
    host: String,
) -> Result<Option<but_github::AuthenticatedUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    let account = but_github::GithubAccountIdentifier::Enterprise { host };
    but_github::get_gh_user(&account, &storage).await
}
