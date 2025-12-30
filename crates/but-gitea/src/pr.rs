use anyhow::{Context as _, Result, bail};

pub async fn list(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    let account_id = resolve_account(preferred_account, owner, storage)?;

    if let Some(access_token) = crate::token::get_gitea_access_token(&account_id, storage)? {
        let client = crate::client::GiteaClient::new(&account_id.host, &access_token)
            .context("Failed to create Gitea client")?;
        let pulls = client
            .list_open_pulls(owner, repo)
            .await
            .context("Failed to list open pull requests")?;
        Ok(pulls)
    } else {
        Ok(vec![])
    }
}

pub async fn create(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    params: crate::client::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let account_id = resolve_account(preferred_account, params.owner, storage)?;
    if let Some(access_token) = crate::token::get_gitea_access_token(&account_id, storage)? {
        let client = crate::client::GiteaClient::new(&account_id.host, &access_token)
            .context("Failed to create Gitea client")?;
        let pr = client
            .create_pull_request(&params)
            .await
            .context("Failed to create pull request")?;
        Ok(pr)
    } else {
        bail!(
            "No Gitea access token found for account '{}'.\nPlease, try to re-authenticate with this account.",
            account_id.username
        );
    }
}

pub async fn get(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: i64,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let account_id = resolve_account(preferred_account, owner, storage)?;
    if let Some(access_token) = crate::token::get_gitea_access_token(&account_id, storage)? {
        let client = crate::client::GiteaClient::new(&account_id.host, &access_token)
            .context("Failed to create Gitea client")?;
        let pr = client
            .get_pull_request(owner, repo, pr_number)
            .await
            .context("Failed to get pull request")?;
        Ok(pr)
    } else {
        bail!(
            "No Gitea access token found for account '{}'.\nPlease, try to re-authenticate with this account.",
            account_id.username
        );
    }
}

fn resolve_account(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    _repo_owner: &str, // Currently we resolve the account by preferred account (if any) or default, and do not use the repo owner.
    storage: &but_forge_storage::Controller,
) -> Result<crate::GiteaAccountIdentifier, anyhow::Error> {
    // We need list_known_gitea_accounts in token.rs
    let known_accounts = crate::token::list_known_gitea_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!("No authenticated Gitea users found. Please authenticate with Gitea first.");
    };

    let account = if let Some(account) = preferred_account {
        if known_accounts.contains(account) {
            account
        } else {
            bail!(
                "Preferred Gitea account '{}' has not authenticated yet. Please choose another account or authenticate with the desired account first.",
                account.username
            );
        }
    } else {
        default_account
    };

    Ok(account.to_owned())
}
