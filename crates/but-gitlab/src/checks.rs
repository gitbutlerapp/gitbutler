use anyhow::{Context as _, Result};

use crate::{GitLabPipelineJob, GitLabProjectId, PipelineLookup, client::GitLabClient};

/// Look up the newest pipeline on a branch and the failed jobs that name its
/// failures, classifying transport failures as `NetworkError` like the other
/// read paths.
pub async fn latest_pipeline_for_branch(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<(PipelineLookup, Vec<GitLabPipelineJob>)> {
    let gl = GitLabClient::from_storage(storage, preferred_account)?;
    async {
        let lookup = gl
            .latest_pipeline_for_branch(project_id.clone(), branch)
            .await?;

        let failed_jobs = match &lookup {
            PipelineLookup::Resolved(Some(pipeline))
                if matches!(
                    &pipeline.status as &str,
                    "failed" | "running" | "canceling" | "canceled"
                ) =>
            {
                gl.failed_jobs(project_id, pipeline.id).await?
            }
            _ => Vec::new(),
        };
        anyhow::Ok((lookup, failed_jobs))
    }
    .await
    .map_err(crate::mr::classify_forge_error)
    .context("Failed to list pipeline checks for branch")
}
