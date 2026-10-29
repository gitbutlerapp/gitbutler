use anyhow::{Context as _, Result};

use crate::client::BitbucketClient;

/// Fetch commit build statuses for a ref, classifying transport failures as
/// `NetworkError` like the other read paths.
///
/// Returns `None` when the ref can't be resolved (e.g. a deleted branch) —
/// an expected "no checks" state the caller must not cache.
pub async fn list_for_ref(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    reference: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Vec<crate::BitbucketBuildStatus>>> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .list_checks_for_ref(workspace, repo_slug, reference)
        .await
        .map_err(crate::pr::classify_forge_error)
        .context("Failed to list checks for ref")
}
