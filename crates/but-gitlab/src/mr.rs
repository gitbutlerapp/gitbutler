use anyhow::{Context as _, Result};

use crate::client::GitLabClient;

pub async fn list(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    if let Ok(gl) = GitLabClient::from_storage(storage, preferred_account) {
        gl.list_open_mrs(project)
            .await
            .context("Failed to list open merge requests")
    } else {
        Ok(vec![])
    }
}

pub async fn list_all_for_target(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project: &str,
    target_branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    if let Ok(gl) = GitLabClient::from_storage(storage, preferred_account) {
        gl.list_mrs_for_target(project, target_branch)
            .await
            .context("Failed to list merge requests for target branch")
    } else {
        Ok(vec![])
    }
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
    project: &str,
    mr_iid: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::MergeRequest> {
    let mr_iid = mr_iid.try_into().context("MR number is too large")?;
    let mr = GitLabClient::from_storage(storage, preferred_account)?
        .get_merge_request(project, mr_iid)
        .await
        .context("Failed to get merge request")?;
    Ok(mr)
}
