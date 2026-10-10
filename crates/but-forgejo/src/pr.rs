use anyhow::{Context as _, Result};

use crate::client::{ForgejoClient, ForgejoPullRequest};

pub async fn list(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgejoPullRequest>> {
    ForgejoClient::from_storage(storage, preferred_account)?
        .list_open_prs(owner, repo)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list open pull requests")
}

pub async fn list_recently_closed(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgejoPullRequest>> {
    ForgejoClient::from_storage(storage, preferred_account)?
        .list_recently_closed_prs(owner, repo)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list recently closed pull requests")
}

pub async fn list_all_for_target(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    target_branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgejoPullRequest>> {
    ForgejoClient::from_storage(storage, preferred_account)?
        .list_prs_for_target(owner, repo, target_branch)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull requests for target branch")
}

pub async fn get(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<ForgejoPullRequest> {
    let number = number.try_into().context("PR number is too large")?;
    ForgejoClient::from_storage(storage, preferred_account)?
        .get_pull_request(owner, repo, number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to get pull request")
}

pub async fn create(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    params: crate::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<ForgejoPullRequest> {
    ForgejoClient::from_storage(storage, preferred_account)?
        .create_pull_request(&params)
        .await
        .context("Failed to create pull request")
}

pub async fn update(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    params: crate::UpdatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<ForgejoPullRequest> {
    ForgejoClient::from_storage(storage, preferred_account)?
        .update_pull_request(&params)
        .await
        .context("Failed to update pull request")
}

pub async fn merge(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    number: usize,
    style: crate::MergeStyle,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let number = number.try_into().context("PR number is too large")?;
    ForgejoClient::from_storage(storage, preferred_account)?
        .merge_pull_request(owner, repo, number, style)
        .await
        .context("Failed to merge pull request")
}

pub async fn set_auto_merge(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    number: usize,
    enabled: bool,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let number = number.try_into().context("PR number is too large")?;
    ForgejoClient::from_storage(storage, preferred_account)?
        .set_pull_request_auto_merge(owner, repo, number, enabled)
        .await
        .context("Failed to set pull request auto-merge state")
}

pub async fn set_draft_state(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    owner: &str,
    repo: &str,
    number: usize,
    is_draft: bool,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let number = number.try_into().context("PR number is too large")?;
    ForgejoClient::from_storage(storage, preferred_account)?
        .set_pull_request_draft_state(owner, repo, number, is_draft)
        .await
        .context("Failed to set pull request draft state")
}

/// Tag transport failures with `but_error::Code::NetworkError` so the desktop
/// can present them appropriately (silent for offline) and cached readers can
/// keep serving the last known data. Only applied to read paths — mutations
/// should still surface failures.
pub(crate) fn classify_forge_error(err: anyhow::Error) -> anyhow::Error {
    if err
        .downcast_ref::<reqwest::Error>()
        .is_some_and(crate::is_network_error)
    {
        return err.context(but_error::Context::new_static(
            but_error::Code::NetworkError,
            "Unable to connect to Forgejo.",
        ));
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_failures_carry_the_network_error_code() {
        // A port that was just proven closed produces the same `reqwest::Error`
        // shape as an unreachable Forgejo host.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let reqwest_err = reqwest::blocking::Client::new()
            .get(format!("http://127.0.0.1:{port}"))
            .send()
            .unwrap_err();
        let err = classify_forge_error(anyhow::Error::from(reqwest_err));
        assert_eq!(
            err.downcast_ref::<but_error::Context>().map(|ctx| ctx.code),
            Some(but_error::Code::NetworkError),
            "cached readers key their stale-data fallback off this code"
        );
    }

    #[test]
    fn api_failures_stay_unclassified() {
        let err = classify_forge_error(anyhow::anyhow!("HTTP 500 Internal Server Error"));
        assert!(
            err.downcast_ref::<but_error::Context>().is_none(),
            "a forge-side failure must not be presented as an offline network"
        );
    }
}
