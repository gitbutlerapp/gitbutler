use anyhow::{Context as _, Result};

/// Fetch repository metadata (fork status, default branch, caller permission)
/// for `workspace/repo_slug`.
pub async fn fetch_repo(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::BitbucketRepo> {
    crate::client::BitbucketClient::from_storage(storage, preferred_account)?
        .fetch_repo(workspace, repo_slug)
        .await
        .context("Failed to fetch Bitbucket repository")
}
