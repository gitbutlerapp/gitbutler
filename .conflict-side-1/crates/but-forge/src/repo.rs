use serde::Serialize;

use crate::ForgeName;

/// Fetch repo-level metadata (currently just permissions + fork bit)
/// for an arbitrary owner/repo on the same forge as the current project.
/// The auth token comes from the project's preferred forge user, which
/// matches how every other forge call in this crate resolves credentials.
pub async fn get_repo_info(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<RepoInfo> {
    let owner = forge_repo_info.owner.as_str();
    let repo = forge_repo_info.repo.as_str();
    match forge_repo_info.forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let gh = but_github::GitHubClient::from_storage(storage, preferred_account)?;
            gh.get_repo(owner, repo).await.map(RepoInfo::from)
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = but_gitlab::GitLabProjectId::new(owner, repo);
            but_gitlab::fetch_project(preferred_account, project_id, storage)
                .await
                .map(RepoInfo::from)
        }
        ForgeName::Bitbucket | ForgeName::Azure => Err(anyhow::anyhow!(
            "Fetching repo info for forge {:?} is not implemented yet.",
            forge_repo_info.forge
        )),
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub permissions: Option<RepoPermissions>,
    pub fork: bool,
    /// Whether the repo deletes the source branch after a PR is merged
    /// (GitHub's per-repo "Automatically delete head branches" setting).
    /// `None` when the field wasn't returned by the forge.
    pub delete_branch_on_merge: Option<bool>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RepoInfo);

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RepoPermissions {
    pub admin: bool,
    pub maintain: bool,
    pub push: bool,
    pub triage: bool,
    pub pull: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RepoPermissions);

impl From<but_github::GitHubRepository> for RepoInfo {
    fn from(value: but_github::GitHubRepository) -> Self {
        RepoInfo {
            permissions: value.permissions.map(|p| RepoPermissions {
                admin: p.admin,
                maintain: p.maintain,
                push: p.push,
                triage: p.triage,
                pull: p.pull,
            }),
            fork: value.fork,
            delete_branch_on_merge: value.delete_branch_on_merge,
        }
    }
}

impl From<but_gitlab::GitLabProject> for RepoInfo {
    fn from(value: but_gitlab::GitLabProject) -> Self {
        // GitLab access levels: 10=Guest, 20=Reporter, 30=Developer,
        // 40=Maintainer, 50=Owner.
        let permissions = value.access_level.map(|level| RepoPermissions {
            pull: level >= 10,
            triage: level >= 20,
            push: level >= 30,
            maintain: level >= 40,
            admin: level >= 50,
        });
        RepoInfo {
            permissions,
            fork: value.forked_from_project_id.is_some(),
            delete_branch_on_merge: value.remove_source_branch_after_merge,
        }
    }
}
