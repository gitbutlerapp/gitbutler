use anyhow::{Context, Result, bail};

pub async fn list(
    preferred_username: &Option<String>,
    owner: &str,
    repo: &str,
) -> Result<Vec<crate::client::PullRequest>> {
    let known_usernames = crate::token::list_known_github_usernames()?;
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

    if let Some(access_token) = crate::token::get_gh_access_token(login)? {
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
