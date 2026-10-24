use anyhow::{Context as _, Result};

use crate::client::ForgejoClient;

/// Fetch the latest commit statuses for a ref, classifying transport failures
/// as `NetworkError` like the other read paths.
///
/// Returns `None` when the ref can't be resolved (e.g. a deleted branch) —
/// an expected "no checks" state the caller must not cache.
pub async fn list_for_ref(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    reference: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Vec<crate::ForgejoCommitStatus>>> {
    ForgejoClient::from_storage(storage, preferred_account)?
        .list_statuses_for_ref(owner, repo, reference)
        .await
        .map_err(crate::pr::classify_forge_error)
        .context("Failed to list commit statuses for ref")
}
