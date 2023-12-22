use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    gb_repository, git,
    project_repository::{self, LogUntil},
    sessions,
};

use super::{errors, get_default_target, Author};

// this struct is a mapping to the view `RemoteBranch` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
//
// it holds data calculated for presentation purposes of one Git branch
// with comparison data to the Target commit, determining if it is mergeable,
// and how far ahead or behind the Target it is.
// an array of them can be requested from the frontend to show in the sidebar
// Tray and should only contain branches that have not been converted into
// virtual branches yet (ie, we have no `Branch` struct persisted in our data.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranch {
    pub sha: git::Oid,
    pub name: git::Refname,
    pub upstream: Option<git::RemoteRefname>,
    pub behind: u32,
    pub commits: Vec<RemoteCommit>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommit {
    pub id: String,
    pub description: String,
    pub created_at: u128,
    pub author: Author,
}

pub fn list_remote_branches(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Vec<RemoteBranch>, errors::ListRemoteBranchesError> {
    // get the current target
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    let default_target = get_default_target(&current_session_reader)
        .context("failed to get default target")?
        .ok_or_else(|| {
            errors::ListRemoteBranchesError::DefaultTargetNotSet(errors::DefaultTargetNotSetError {
                project_id: project_repository.project().id,
            })
        })?;

    let remote_branches = project_repository
        .git_repository
        .branches(Some(git2::BranchType::Remote))
        .context("failed to list remove branches")?
        .flatten()
        .map(|(branch, _)| branch)
        .map(|branch| branch_to_remote_branch(project_repository, &branch, default_target.sha))
        .collect::<Result<Vec<_>>>()
        .context("failed to convert branches")?
        .into_iter()
        .flatten()
        .filter(|branch| branch.name.branch() != Some(default_target.branch.branch()))
        .collect::<Vec<_>>();

    Ok(remote_branches)
}

pub fn branch_to_remote_branch(
    project_repository: &project_repository::Repository,
    branch: &git::Branch,
    base: git::Oid,
) -> Result<Option<RemoteBranch>> {
    branch
        .target()
        .map(|sha| {
            let ahead = project_repository
                .log(sha, LogUntil::Commit(base))
                .context("failed to get ahead commits")?;

            let name = git::Refname::try_from(branch).context("could not get branch name")?;

            let count_behind = project_repository
                .distance(base, sha)
                .context("failed to get behind count")?;

            Ok(RemoteBranch {
                sha,
                upstream: if let git::Refname::Local(local_name) = &name {
                    local_name.remote().cloned()
                } else {
                    None
                },
                name,
                behind: count_behind,
                commits: ahead
                    .into_iter()
                    .map(|commit| commit_to_remote_commit(&commit))
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .transpose()
}

pub fn commit_to_remote_commit(commit: &git::Commit) -> Result<RemoteCommit> {
    Ok(RemoteCommit {
        id: commit.id().to_string(),
        description: commit.message().unwrap_or_default().to_string(),
        created_at: commit.time().seconds().try_into().unwrap(),
        author: commit.author().into(),
    })
}
