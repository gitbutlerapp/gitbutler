use std::path::Path;

use crate::author::Author;
use anyhow::{Context, Result};
use gitbutler_branch::ReferenceExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{LogUntil, RepoActionsExt, RepositoryExt};
use gitbutler_serde::BStringForFrontend;
use gitbutler_stack::{Target, VirtualBranchesHandle};
use serde::Serialize;

/// this struct is a mapping to the view `RemoteBranch` type in Typescript
/// found in src-tauri/src/routes/repo/[project_id]/types.ts
///
/// it holds data calculated for presentation purposes of one Git branch
/// with comparison data to the Target commit, determining if it is mergeable,
/// and how far ahead or behind the Target it is.
/// an array of them can be requested from the frontend to show in the sidebar
/// Tray and should only contain branches that have not been converted into
/// virtual branches yet (ie, we have no `Branch` struct persisted in our data.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranch {
    #[serde(with = "gitbutler_serde::oid")]
    pub sha: git2::Oid,
    pub name: Refname,
    pub upstream: Option<RemoteRefname>,
    pub given_name: String,
    pub last_commit_timestamp_ms: Option<u128>,
    pub last_commit_author: Option<String>,
    pub is_remote: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchData {
    #[serde(with = "gitbutler_serde::oid")]
    pub sha: git2::Oid,
    pub name: Refname,
    pub upstream: Option<RemoteRefname>,
    pub behind: u32,
    pub commits: Vec<RemoteCommit>,
    #[serde(with = "gitbutler_serde::oid_opt", default)]
    pub fork_point: Option<git2::Oid>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommit {
    pub id: String,
    pub description: BStringForFrontend,
    pub created_at: u128,
    pub author: Author,
    pub change_id: Option<String>,
    #[serde(with = "gitbutler_serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
    pub conflicted: bool,
}

/// Return information on all local branches, while skipping gitbutler-specific branches in `refs/heads`.
///
/// Note to be confused with `list_branches()`, which is used for the new branch listing.
///
/// # Previous notes
/// For legacy purposes, this is still named "remote" branches, but it's actually
/// a list of all the normal (non-gitbutler) git branches.
pub fn list_local_branches(ctx: &CommandContext) -> Result<Vec<RemoteBranch>> {
    let default_target = default_target(&ctx.project().gb_dir())?;

    let mut remote_branches = vec![];
    let remotes = ctx.repository().remotes()?;
    for (branch, _) in ctx
        .repository()
        .branches(None)
        .context("failed to list remote branches")?
        .flatten()
    {
        let branch = match branch_to_remote_branch(&branch, &remotes) {
            Ok(Some(b)) => b,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(?err, "Ignoring branch");
                continue;
            }
        };
        let branch_is_trunk = branch.name.branch() == Some(default_target.branch.branch())
            && branch.name.remote() == Some(default_target.branch.remote());

        if !branch_is_trunk
            && branch.name.branch() != Some("gitbutler/integration") // Remove after rename migration complete.
            && branch.name.branch() != Some("gitbutler/workspace")
            && branch.name.branch() != Some("gitbutler/edit")
            && branch.name.branch() != Some("gitbutler/target")
        {
            remote_branches.push(branch);
        }
    }
    Ok(remote_branches)
}

pub(crate) fn get_branch_data(ctx: &CommandContext, refname: &Refname) -> Result<RemoteBranchData> {
    let default_target = default_target(&ctx.project().gb_dir())?;

    let branch = ctx
        .repository()
        .maybe_find_branch_by_refname(refname)?
        .ok_or(anyhow::anyhow!("failed to find branch {}", refname))?;

    branch_to_remote_branch_data(ctx, &branch, default_target.sha)?
        .context("failed to get branch data")
}

pub(crate) fn get_commit_data(
    ctx: &CommandContext,
    sha: git2::Oid,
) -> Result<Option<RemoteCommit>> {
    let commit = match ctx.repository().find_commit(sha) {
        Ok(commit) => commit,
        Err(error) => {
            if error.code() == git2::ErrorCode::NotFound {
                return Ok(None);
            } else {
                anyhow::bail!(error);
            }
        }
    };
    Ok(Some(commit_to_remote_commit(&commit)))
}

pub(crate) fn branch_to_remote_branch(
    branch: &git2::Branch<'_>,
    remotes: &git2::string_array::StringArray,
) -> Result<Option<RemoteBranch>> {
    let commit = branch.get().peel_to_commit()?;
    let name = Refname::try_from(branch).context("could not get branch name")?;

    let given_name = branch.get().given_name(remotes)?;

    Ok(branch.get().target().map(|sha| RemoteBranch {
        sha,
        upstream: if let Refname::Local(local_name) = &name {
            local_name.remote().cloned()
        } else {
            None
        },
        name,
        given_name,
        last_commit_timestamp_ms: commit
            .time()
            .seconds()
            .try_into()
            .map(|t: u128| t * 1000)
            .ok(),
        last_commit_author: commit.author().name().map(std::string::ToString::to_string),
        is_remote: branch.get().is_remote(),
    }))
}

pub(crate) fn branch_to_remote_branch_data(
    ctx: &CommandContext,
    branch: &git2::Branch,
    base: git2::Oid,
) -> Result<Option<RemoteBranchData>> {
    branch
        .get()
        .target()
        .map(|sha| {
            let ahead = ctx
                .repository()
                .log(sha, LogUntil::Commit(base))
                .context("failed to get ahead commits")?;

            let name = Refname::try_from(branch).context("could not get branch name")?;

            let count_behind = ctx
                .distance(base, sha)
                .context("failed to get behind count")?;

            let fork_point = ahead.last().and_then(|c| c.parent(0).ok()).map(|c| c.id());

            Ok(RemoteBranchData {
                sha,
                upstream: if let Refname::Local(local_name) = &name {
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

pub(crate) fn commit_to_remote_commit(commit: &git2::Commit) -> RemoteCommit {
    let parent_ids = commit.parents().map(|c| c.id()).collect();
    RemoteCommit {
        id: commit.id().to_string(),
        description: commit.message_bstr().into(),
        created_at: commit.time().seconds().try_into().unwrap(),
        author: commit.author().into(),
        change_id: commit.change_id(),
        parent_ids,
        conflicted: commit.is_conflicted(),
    }
}

fn default_target(base_path: &Path) -> Result<Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}
