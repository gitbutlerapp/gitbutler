use anyhow::{Context as _, Result};

use crate::client::GitHubClient;

pub async fn list(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    if let Ok(gh) = GitHubClient::from_storage(storage, preferred_account) {
        gh.list_open_pulls(owner, repo)
            .await
            .context("Failed to list open pull requests")
    } else {
        Ok(vec![])
    }
}

pub async fn create(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    params: crate::client::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr = GitHubClient::from_storage(storage, preferred_account)?
        .create_pull_request(&params)
        .await
        .context("Failed to create pull request")?;
    Ok(pr)
}

pub async fn get(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    let pr = GitHubClient::from_storage(storage, preferred_account)?
        .get_pull_request(owner, repo, pr_number)
        .await
        .context("Failed to get pull request")?;
    Ok(pr)
}
