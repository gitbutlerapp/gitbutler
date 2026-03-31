use anyhow::{Context as _, Result};

use crate::client::GiteaClient;

pub async fn list(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    if let Ok(gitea) = GiteaClient::from_storage(storage, preferred_account) {
        gitea
            .list_open_pulls(owner, repo)
            .await
            .context("Failed to list open pull requests")
    } else {
        Ok(vec![])
    }
}

pub async fn list_all_for_branch(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    owner: &str,
    repo: &str,
    branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    if let Ok(gitea) = GiteaClient::from_storage(storage, preferred_account) {
        gitea
            .list_pulls_for_base(owner, repo, branch)
            .await
            .context("Failed to list pull requests for branch")
    } else {
        Ok(vec![])
    }
}

pub async fn create(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    params: crate::client::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr = GiteaClient::from_storage(storage, preferred_account)?
        .create_pull_request(&params)
        .await
        .context("Failed to create pull request")?;
    Ok(pr)
}

pub async fn get(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    let pr = GiteaClient::from_storage(storage, preferred_account)?
        .get_pull_request(owner, repo, pr_number)
        .await
        .context("Failed to get pull request")?;
    Ok(pr)
}

pub async fn update(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    params: crate::client::UpdatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr = GiteaClient::from_storage(storage, preferred_account)?
        .update_pull_request(&params)
        .await
        .context("Failed to update pull request")?;
    Ok(pr)
}

pub async fn merge(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    params: crate::client::MergePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GiteaClient::from_storage(storage, preferred_account)?
        .merge_pull_request(&params)
        .await
        .context("Failed to merge PR")
}

pub async fn set_draft_state(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    params: crate::client::SetPullRequestDraftStateParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GiteaClient::from_storage(storage, preferred_account)?
        .set_pull_request_draft_state(&params)
        .await
        .context("Failed to update PR draft state")
}

pub async fn set_auto_merge(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    params: crate::client::SetPullRequestAutoMergeParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GiteaClient::from_storage(storage, preferred_account)?
        .set_pull_request_auto_merge(&params)
        .await
        .context("Failed to update PR auto-merge state")
}
