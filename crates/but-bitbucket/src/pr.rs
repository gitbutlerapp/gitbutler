use anyhow::{Context as _, Result};

use crate::client::BitbucketClient;

pub async fn list(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::BitbucketPullRequest>> {
    if let Ok(bb) = BitbucketClient::from_storage(storage, preferred_account) {
        bb.list_open_prs(workspace, repo_slug)
            .await
            .context("Failed to list open pull requests")
    } else {
        Ok(vec![])
    }
}

pub async fn list_all_for_target(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    target_branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::BitbucketPullRequest>> {
    if let Ok(bb) = BitbucketClient::from_storage(storage, preferred_account) {
        bb.list_prs_for_target(workspace, repo_slug, target_branch)
            .await
            .context("Failed to list pull requests for target branch")
    } else {
        Ok(vec![])
    }
}

pub async fn get(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    id: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::BitbucketPullRequest> {
    let id = id.try_into().context("PR number is too large")?;
    BitbucketClient::from_storage(storage, preferred_account)?
        .get_pull_request(workspace, repo_slug, id)
        .await
        .context("Failed to get pull request")
}

pub async fn create(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    params: crate::client::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::BitbucketPullRequest> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .create_pull_request(&params)
        .await
        .context("Failed to create pull request")
}

pub async fn update(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    params: crate::client::UpdatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::BitbucketPullRequest> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .update_pull_request(&params)
        .await
        .context("Failed to update pull request")
}

pub async fn merge(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    params: crate::client::MergePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .merge_pull_request(&params)
        .await
        .context("Failed to merge pull request")
}

pub async fn decline(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    id: usize,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let id = id.try_into().context("PR number is too large")?;
    BitbucketClient::from_storage(storage, preferred_account)?
        .decline_pull_request(workspace, repo_slug, id)
        .await
        .context("Failed to decline pull request")
}

pub async fn set_draft_state(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    params: crate::client::SetPullRequestDraftStateParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .set_pull_request_draft_state(&params)
        .await
        .context("Failed to set pull request draft state")
}
