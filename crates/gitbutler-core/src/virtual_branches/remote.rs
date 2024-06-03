use std::path::Path;

use anyhow::{Context, Result};
use bstr::BString;
use serde::Serialize;

use super::{target, Author, VirtualBranchesHandle};
use crate::{
    git::{self, CommitExt},
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
    pub fork_point: Option<git::Oid>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommit {
    pub id: String,
    #[serde(serialize_with = "crate::serde::as_string_lossy")]
    pub description: BString,
    pub created_at: u128,
    pub author: Author,
    pub change_id: Option<String>,
    pub parent_ids: Vec<git::Oid>,
}

// for legacy purposes, this is still named "remote" branches, but it's actually
// a list of all the normal (non-gitbutler) git branches.
pub fn list_remote_branches(
    project_repository: &project_repository::Repository,
) -> Result<Vec<RemoteBranch>> {
    let default_target = default_target(&project_repository.project().gb_dir())?;

    let mut remote_branches = vec![];
    for branch in project_repository
        .git_repository
        .branches(None)
        .context("failed to list remote branches")?
        .flatten()
    {
        let branch = branch_to_remote_branch(&branch)?;

        if let Some(branch) = branch {
            let branch_is_trunk = branch.name.branch() == Some(default_target.branch.branch())
                && branch.name.remote() == Some(default_target.branch.remote());

            if !branch_is_trunk
                && branch.name.branch() != Some("gitbutler/integration")
                && branch.name.branch() != Some("gitbutler/target")
            {
                remote_branches.push(branch);
            }
        }
    }
    Ok(remote_branches)
}

pub fn get_branch_data(
    project_repository: &project_repository::Repository,
    refname: &git::Refname,
) -> Result<RemoteBranchData> {
    let default_target = default_target(&project_repository.project().gb_dir())?;

    let branch = project_repository
        .git_repository
        .find_branch(refname)
        .context(format!("failed to find branch with refname {refname}"))?;

    branch_to_remote_branch_data(project_repository, &branch, default_target.sha)?
        .context("failed to get branch data")
}

pub fn branch_to_remote_branch(branch: &git2::Branch) -> Result<Option<RemoteBranch>> {
    let commit = match branch.get().peel_to_commit() {
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
    let name = git::Refname::try_from(branch).context("could not get branch name");
    match name {
        Ok(name) => branch
            .get()
            .target()
            .map(|sha| {
                Ok(RemoteBranch {
                    sha: sha.into(),
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
                    last_commit_author: commit
                        .author()
                        .name()
                        .map(std::string::ToString::to_string),
                })
            })
            .transpose(),
        Err(_) => Ok(None),
    }
}

pub fn branch_to_remote_branch_data(
    project_repository: &project_repository::Repository,
    branch: &git2::Branch,
    base: git::Oid,
) -> Result<Option<RemoteBranchData>> {
    branch
        .get()
        .target()
        .map(|sha| {
            let ahead = project_repository
                .log(sha.into(), LogUntil::Commit(base))
                .context("failed to get ahead commits")?;

            let name = git::Refname::try_from(branch).context("could not get branch name")?;

            let count_behind = project_repository
                .distance(base, sha.into())
                .context("failed to get behind count")?;

            let fork_point = ahead
                .last()
                .and_then(|c| c.parent(0).ok())
                .map(|c| c.id().into());

            Ok(RemoteBranchData {
                sha: sha.into(),
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
                fork_point,
            })
        })
        .transpose()
}

pub fn commit_to_remote_commit(commit: &git2::Commit) -> RemoteCommit {
    let parent_ids: Vec<git::Oid> = commit
        .parents()
        .map(|c| git::Oid::from(c.id()))
        .collect::<Vec<_>>();
    RemoteCommit {
        id: commit.id().to_string(),
        description: commit.message_bstr().to_owned(),
        created_at: commit.time().seconds().try_into().unwrap(),
        author: commit.author().into(),
        change_id: commit.change_id(),
        parent_ids,
    }
}

fn default_target(base_path: &Path) -> Result<target::Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}
