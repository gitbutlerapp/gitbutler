use anyhow::{Context as _, Result};

use crate::{GitLabProjectId, client::GitLabClient};

pub async fn list(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    if let Ok(gl) = GitLabClient::from_storage(storage, preferred_account) {
        gl.list_open_mrs(project_id)
            .await
            .context("Failed to list open merge requests")
    } else {
        Ok(vec![])
    }
}

pub async fn list_all_for_target(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    target_branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::MergeRequest>> {
    if let Ok(gl) = GitLabClient::from_storage(storage, preferred_account) {
        gl.list_mrs_for_target(project_id, target_branch)
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
    project_id: GitLabProjectId,
    mr_iid: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::MergeRequest> {
    let mr_iid = mr_iid.try_into().context("MR number is too large")?;
    let mr = GitLabClient::from_storage(storage, preferred_account)?
        .get_merge_request(project_id, mr_iid)
        .await
        .context("Failed to get merge request")?;
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
