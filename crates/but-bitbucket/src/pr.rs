use anyhow::{Context as _, Result};

use crate::client::BitbucketClient;

pub async fn list(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::BitbucketPullRequest>> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .list_open_prs(workspace, repo_slug)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list open pull requests")
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
            "Unable to connect to Bitbucket.",
        ));
    }
    err
}

pub async fn list_recently_closed(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::BitbucketPullRequest>> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .list_recently_closed_prs(workspace, repo_slug)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list recently closed pull requests")
}

pub async fn list_all_for_target(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    workspace: &str,
    repo_slug: &str,
    target_branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::BitbucketPullRequest>> {
    BitbucketClient::from_storage(storage, preferred_account)?
        .list_prs_for_target(workspace, repo_slug, target_branch)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull requests for target branch")
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
        .map_err(classify_forge_error)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Serve one request with the given raw HTTP response, then hang up.
    fn one_shot_server(response: &'static [u8]) -> std::net::SocketAddr {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(response);
        });
        addr
    }

    #[test]
    fn interrupted_response_bodies_carry_the_network_error_code() {
        // The response promises more bytes than arrive before the connection
        // closes — the shape of a network dropping mid-transfer.
        let addr = one_shot_server(
            b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 100\r\n\r\n[{\"id\"",
        );
        let reqwest_err = reqwest::blocking::get(format!("http://{addr}/"))
            .unwrap()
            .json::<serde_json::Value>()
            .unwrap_err();
        let err = classify_forge_error(anyhow::Error::from(reqwest_err));
        assert_eq!(
            err.downcast_ref::<but_error::Context>().map(|ctx| ctx.code),
            Some(but_error::Code::NetworkError),
            "a connection lost while reading the body is an outage like any other"
        );
    }

    #[test]
    fn malformed_payloads_stay_unclassified() {
        // The body arrives in full but is not JSON — a forge-side problem,
        // not an outage.
        let addr = one_shot_server(
            b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 12\r\n\r\nnot-json-12b",
        );
        let reqwest_err = reqwest::blocking::get(format!("http://{addr}/"))
            .unwrap()
            .json::<serde_json::Value>()
            .unwrap_err();
        let err = classify_forge_error(anyhow::Error::from(reqwest_err));
        assert!(
            err.downcast_ref::<but_error::Context>().is_none(),
            "an unparseable payload must not be presented as an offline network"
        );
    }

    #[test]
    fn connection_failures_carry_the_network_error_code() {
        // Binding an ephemeral port and dropping the listener leaves a port
        // that was just proven closed; connecting to it produces the same
        // `reqwest::Error` shape as an unreachable Bitbucket host.
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
        let err = classify_forge_error(anyhow::anyhow!(
            "Failed to list open pull requests: 500 Internal Server Error"
        ));
        assert!(
            err.downcast_ref::<but_error::Context>().is_none(),
            "a forge-side failure must not be presented as an offline network"
        );
    }
}
