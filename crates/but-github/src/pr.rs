use anyhow::{Context, Result, bail};

pub async fn list(
    preferred_username: &Option<String>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::controller::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    let username = resolve_username(preferred_username, storage)?;
    let account_id = crate::token::GithubAccountIdentifier::oauth(&username);

    if let Some(access_token) = crate::token::get_gh_access_token(&account_id, storage)? {
        let gh = crate::client::GitHubClient::new(&access_token)
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
    preferred_username: &Option<String>,
    params: crate::client::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<crate::client::PullRequest> {
    let username = resolve_username(preferred_username, storage)?;
    let account_id = crate::token::GithubAccountIdentifier::oauth(&username);

    if let Some(access_token) = crate::token::get_gh_access_token(&account_id, storage)? {
        let gh = crate::client::GitHubClient::new(&access_token)
            .context("Failed to create GitHub client")?;
        let pr = gh
            .create_pull_request(&params)
            .await
            .context("Failed to create pull request")?;
        Ok(pr)
    } else {
        bail!("No GitHub access token found for user '{}'", username);
    }
}

fn resolve_username(
    preferred_username: &Option<String>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<String, anyhow::Error> {
    let known_usernames = crate::token::list_known_github_accounts(storage)?;
    let Some(default_username) = known_usernames.first() else {
        bail!("No authenticated GitHub users found. Please authenticate with GitHub first.");
    };
    let login = if let Some(username) = preferred_username {
        if known_usernames.contains(username) {
            username
        } else {
            bail!(
                "Preferred GitHub username '{}' has not authenticated yet. Please choose another username or authenticate with the desired account first.",
                username
            );
        }
    } else {
        default_username
    };

    Ok(login.to_owned())
}
