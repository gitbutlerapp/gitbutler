use serde::Serialize;

use crate::ForgeName;

/// Fetch repo-level metadata (currently just permissions + fork bit)
/// for an arbitrary owner/repo on the same forge as the current project.
/// Credentials must belong to a stored account matching the repository's host
/// and provider. The project's preferred forge user is used when it matches.
pub async fn get_repo_info(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<RepoInfo> {
    let preferred_forge_user =
        crate::resolve_account(preferred_forge_user, forge_repo_info, storage)?;
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
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            but_bitbucket::fetch_repo(preferred_account, owner, repo, storage)
                .await
                .map(RepoInfo::from)
        }
        ForgeName::Azure => Err(anyhow::anyhow!(
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
    /// Whether the repo is visible only to people granted access.
    /// `None` when the forge does not report it — Azure, or a GitLab token
    /// without the scope to read project metadata.
    pub private: Option<bool>,
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
            private: Some(value.private),
            delete_branch_on_merge: value.delete_branch_on_merge,
        }
    }
}

impl From<but_bitbucket::BitbucketRepo> for RepoInfo {
    fn from(value: but_bitbucket::BitbucketRepo) -> Self {
        // Bitbucket permissions are `admin` > `write` > `read`. `value.permission`
        // is `None` when it can't be determined — Bitbucket's
        // `/user/permissions/repositories` returns no row for access inherited via
        // workspace/group membership, and the call may lack scope or fail. We always
        // produce `Some(RepoPermissions)`, defaulting that unknown case to optimistic
        // (push allowed) so the merge button isn't wrongly disabled; Bitbucket
        // enforces permissions server-side.
        let permissions = Some(match value.permission.as_deref() {
            Some("admin") => RepoPermissions {
                admin: true,
                maintain: true,
                push: true,
                triage: true,
                pull: true,
            },
            Some("write") => RepoPermissions {
                admin: false,
                maintain: false,
                push: true,
                triage: true,
                pull: true,
            },
            Some("read") => RepoPermissions {
                admin: false,
                maintain: false,
                push: false,
                triage: false,
                pull: true,
            },
            _ => RepoPermissions {
                admin: false,
                maintain: false,
                push: true,
                triage: true,
                pull: true,
            },
        });
        RepoInfo {
            permissions,
            fork: value.is_fork,
            private: Some(value.is_private),
            // Bitbucket has no per-repo "delete source branch after merge" flag
            // exposed on the repository object.
            delete_branch_on_merge: None,
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
            // Everything GitLab does not call `public` is restricted to some
            // group of people, so `internal` counts as private here.
            private: value
                .visibility
                .as_deref()
                .map(|visibility| visibility != "public"),
            delete_branch_on_merge: value.remove_source_branch_after_merge,
        }
    }
}
