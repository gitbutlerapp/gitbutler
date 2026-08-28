use anyhow::{Context as _, Result};

use crate::{GitLabProjectId, client::GitLabClient};

/// Fetch pipeline jobs for a ref, classifying transport failures as
/// `NetworkError` like the other read paths.
pub async fn list_pipeline_jobs_for_ref(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    reference: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::GitLabPipelineJob>> {
    GitLabClient::from_storage(storage, preferred_account)?
        .list_pipeline_jobs_for_ref(project_id, reference)
        .await
        .map_err(crate::mr::classify_forge_error)
        .context("Failed to list pipeline jobs for ref")
}
