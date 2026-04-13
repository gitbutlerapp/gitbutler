use anyhow::{Context as _, Result};

use crate::client::GiteaClient;

pub async fn list(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    if let Ok(gt) = GiteaClient::from_storage(storage, preferred_account) {
        gt.list_open_pulls(owner, repo)
            .await
            .context("Failed to list open pull requests")
    } else {
        Ok(vec![])
    }
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
