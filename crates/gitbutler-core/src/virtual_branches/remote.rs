use std::path::Path;

use anyhow::{Context, Result};
use bstr::BString;
use serde::Serialize;

use super::{errors, target, Author, VirtualBranchesHandle};
use crate::{
    git,
    project_repository::{self, LogUntil},
};

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
    pub last_commit_timestamp_ms: Option<u128>,
    pub last_commit_author: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchData {
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
    #[serde(serialize_with = "crate::serde::as_string_lossy")]
    pub description: BString,
    pub created_at: u128,
    pub author: Author,
}

pub fn list_remote_branches(
    project_repository: &project_repository::Repository,
) -> Result<Vec<RemoteBranch>, errors::ListRemoteBranchesError> {
    let default_target = default_target(&project_repository.project().gb_dir())?;

    let remote_branches = project_repository
        .git_repository
        .branches(Some(git2::BranchType::Remote))
        .context("failed to list remote branches")?
        .flatten()
        .map(|(branch, _)| branch)
        .map(|branch| branch_to_remote_branch(&branch))
        .collect::<Result<Vec<_>>>()
        .context("failed to convert branches")?
        .into_iter()
        .flatten()
        .filter(|branch| branch.name.branch() != Some(default_target.branch.branch()))
        .collect::<Vec<_>>();

    Ok(remote_branches)
}

pub fn get_branch_data(
    project_repository: &project_repository::Repository,
    refname: &git::Refname,
) -> Result<super::RemoteBranchData, errors::GetRemoteBranchDataError> {
    let default_target = default_target(&project_repository.project().gb_dir())?;

    let branch = project_repository
        .git_repository
        .find_branch(refname)
        .context(format!("failed to find branch with refname {refname}"))?;

    let branch_data = branch_to_remote_branch_data(project_repository, &branch, default_target.sha)
        .context("failed to get branch data")?;

    branch_data
        .ok_or_else(|| {
            errors::GetRemoteBranchDataError::Other(anyhow::anyhow!("no data found for branch"))
        })
        .map(|branch_data| RemoteBranchData {
            sha: branch_data.sha,
            name: branch_data.name,
            upstream: branch_data.upstream,
            behind: branch_data.behind,
            commits: branch_data.commits,
        })
}

pub fn branch_to_remote_branch(branch: &git::Branch) -> Result<Option<RemoteBranch>> {
    let commit = match branch.peel_to_commit() {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(
                ?err,
                "ignoring branch {:?} as peeling failed",
                branch.name()
            );
            return Ok(None);
        }
    };
    branch
        .target()
        .map(|sha| {
            let name = git::Refname::try_from(branch).context("could not get branch name")?;
            Ok(RemoteBranch {
                sha,
                upstream: if let git::Refname::Local(local_name) = &name {
                    local_name.remote().cloned()
                } else {
                    None
                },
                name,
                last_commit_timestamp_ms: commit
                    .time()
                    .seconds()
                    .try_into()
                    .map(|t: u128| t * 1000)
                    .ok(),
                last_commit_author: commit.author().name().map(std::string::ToString::to_string),
            })
        })
        .transpose()
}

pub fn branch_to_remote_branch_data(
    project_repository: &project_repository::Repository,
    branch: &git::Branch,
    base: git::Oid,
) -> Result<Option<RemoteBranchData>> {
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

            Ok(RemoteBranchData {
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
                    .collect::<Vec<_>>(),
            })
        })
        .transpose()
}

pub fn commit_to_remote_commit(commit: &git::Commit) -> RemoteCommit {
    RemoteCommit {
        id: commit.id().to_string(),
        description: commit.message().to_owned(),
        created_at: commit.time().seconds().try_into().unwrap(),
        author: commit.author().into(),
    }
}

fn default_target(base_path: &Path) -> Result<target::Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}
