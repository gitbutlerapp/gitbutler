use anyhow::{Context as _, Result};

use crate::{GitLabProjectId, client::GitLabClient};

pub async fn list(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    GitLabClient::from_storage(storage, preferred_account)?
        .list_open_mrs(project_id)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list open merge requests")
}

pub async fn list_recently_closed(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    GitLabClient::from_storage(storage, preferred_account)?
        .list_recently_closed_mrs(project_id)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list recently closed merge requests")
}

pub async fn list_all_for_target(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    target_branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    GitLabClient::from_storage(storage, preferred_account)?
        .list_mrs_for_target(project_id, target_branch)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list merge requests for target branch")
}

pub async fn list_for_commit(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    commit_sha: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    GitLabClient::from_storage(storage, preferred_account)?
        .list_mrs_for_commit(project_id, commit_sha)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list merge requests for commit")
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
            "Unable to connect to GitLab.",
        ));
    }
    err
}

pub async fn create(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    params: crate::client::CreateMergeRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::MergeRequest> {
    let mr = GitLabClient::from_storage(storage, preferred_account)?
        .create_merge_request(&params)
        .await
        .context("Failed to create merge request")?;
    Ok(mr)
}

pub async fn get(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    mr_iid: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::MergeRequest> {
    let mr_iid = mr_iid.try_into().context("MR number is too large")?;
    let mr = GitLabClient::from_storage(storage, preferred_account)?
        .get_merge_request(project_id, mr_iid)
        .await
        .map_err(classify_forge_error)
        .context("Failed to get merge request")?;
    Ok(mr)
}

pub async fn update(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    params: crate::client::UpdateMergeRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::MergeRequest> {
    let mr = GitLabClient::from_storage(storage, preferred_account)?
        .update_merge_request(&params)
        .await
        .context("Failed to update merge request")?;
    Ok(mr)
}

pub async fn merge(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    params: crate::client::MergeMergeRequestParams,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitLabClient::from_storage(storage, preferred_account)?
        .merge_merge_request(&params)
        .await
        .context("Faile to merge MR")
}

pub async fn set_draft_state(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    params: crate::client::SetMergeRequestDraftStateParams,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitLabClient::from_storage(storage, preferred_account)?
        .set_merge_request_draft_state(&params)
        .await
        .context("Failed to set MR draft state")
}

pub async fn set_auto_merge(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    params: crate::client::SetMergeRequestAutoMergeParams,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitLabClient::from_storage(storage, preferred_account)?
        .set_merge_request_auto_merge(&params)
        .await
        .context("Failed to set MR auto-merge state")
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
            b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 100\r\n\r\n[{\"iid\"",
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
        // `reqwest::Error` shape as an unreachable GitLab host.
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
            "Failed to list open merge requests: 500 Internal Server Error"
        ));
        assert!(
            err.downcast_ref::<but_error::Context>().is_none(),
            "a forge-side failure must not be presented as an offline network"
        );
    }
}
