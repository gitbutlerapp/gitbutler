use std::fmt::Display;

use anyhow::Context;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLabProjectId {
    /// The username or group name that owns the project.
    ///
    /// This should be the the namespace of the project in the git remote.
    username: String,
    /// The name of the project
    ///
    /// This should be the name of the project in the git remote, without the namespace.
    project_name: String,
}

impl GitLabProjectId {
    pub fn new(username: &str, project_name: &str) -> Self {
        GitLabProjectId {
            username: username.to_string(),
            project_name: project_name.to_string(),
        }
    }
}

impl Display for GitLabProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let project_id = format!("{}/{}", self.username, self.project_name);
        let encoded = urlencoding::encode(&project_id);
        write!(f, "{encoded}")
    }
}

/// Fetch the GitLab project information by its ID.
///
/// Can be used to resolve a numeric ID from an encoded one.
pub async fn fetch_project(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    project_id: GitLabProjectId,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<crate::client::GitLabProject> {
    crate::GitLabClient::from_storage(storage, preferred_account)?
        .fetch_project(project_id)
        .await
        .context("Failed to set MR auto-merge state")
}
