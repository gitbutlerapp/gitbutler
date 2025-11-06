use anyhow::{Context, Result, bail};

pub async fn list(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::controller::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    let account_id = resolve_account(preferred_account, storage)?;
    if let Some(access_token) = crate::token::get_gh_access_token(&account_id, storage)? {
        let gh = account_id
            .client(&access_token)
            .context("Failed to create GitHub client")?;
        let pulls = gh
            .list_open_pulls(owner, repo)
            .await
            .context("Failed to list open pull requests")?;
        Ok(pulls)
    } else {
        Ok(vec![])
    }
}

pub async fn create(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    params: crate::client::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<crate::client::PullRequest> {
    let account_id = resolve_account(preferred_account, storage)?;
    if let Some(access_token) = crate::token::get_gh_access_token(&account_id, storage)? {
        let gh = account_id
            .client(&access_token)
            .context("Failed to create GitHub client")?;
        let pr = gh
            .create_pull_request(&params)
            .await
            .context("Failed to create pull request")?;
        Ok(pr)
    } else {
        bail!(
            "No GitHub access token found for account '{}'.\nPlease, try to re-authenticate with this account.",
            account_id
        );
    }
}

fn resolve_account(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<crate::GithubAccountIdentifier, anyhow::Error> {
    let known_accounts = crate::token::list_known_github_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!("No authenticated GitHub users found. Please authenticate with GitHub first.");
    };
    let account = if let Some(account) = preferred_account {
        if known_accounts.contains(account) {
            account
        } else {
            bail!(
                "Preferred GitHub account '{}' has not authenticated yet. Please choose another account or authenticate with the desired account first.",
                account
            );
        }
    } else {
        default_account
    };

    Ok(account.to_owned())
}
