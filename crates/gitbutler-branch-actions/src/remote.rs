use std::path::Path;

use crate::author::Author;
use anyhow::{Context, Result};
use git2::BranchType;
use gitbutler_branch::ReferenceExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{LogUntil, RepositoryExt};
use gitbutler_repo_actions::RepoActionsExt;
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

/// Finds all branches matching a given name, which can be at most one local branch,
/// and any number of branches (on different remotes).
///
/// # Previous notes
/// For legacy purposes, this is still named "remote" branches, but it's actually
/// a list of all the normal (non-gitbutler) git branches.
pub fn find_git_branches(ctx: &CommandContext, branch_name: &str) -> Result<Vec<RemoteBranch>> {
    let repo = ctx.repository();
    let remotes_raw = repo.remotes()?;
    let remotes: Vec<_> = remotes_raw.iter().flatten().collect();

    // We since we are testing for the presence of branches we swallow any errors
    // from both finding the branch, and looking up the branch commit data. The
    // latter can fail if/when a ref points to a object that doesn't exist.
    let mut all_branches: Vec<RemoteBranch> = remotes
        .iter()
        .filter_map(|remote_name| {
            let branch_name = format!("{}/{}", remote_name, branch_name);
            let blah = repo
                .find_branch(&branch_name, BranchType::Remote)
                .ok()
                .and_then(|branch| branch_to_remote_branch(&branch, &[remote_name]).ok());
            blah
        })
        .collect();

    if let Some(local_branch) = repo
        .find_branch(branch_name, BranchType::Local)
        .ok()
        .and_then(|branch| branch_to_remote_branch(&branch, &remotes).ok())
    {
        all_branches.push(local_branch);
    }

    let target_branch = &default_target(&ctx.project().gb_dir())?.branch;
    Ok(all_branches
        .into_iter()
        .filter(|branch| {
            branch.name != target_branch.into() &&
            branch.name.branch() != Some("gitbutler/integration") && // Remove after rename migration complete.
            branch.name.branch() != Some("gitbutler/workspace") &&
            branch.name.branch() != Some("gitbutler/target") &&
            branch.name.branch() != Some("gitbutler/edit")
        })
        .collect())
}

pub(crate) fn get_branch_data(ctx: &CommandContext, refname: &Refname) -> Result<RemoteBranchData> {
    let branch = ctx
        .repository()
        .maybe_find_branch_by_refname(refname)?
        .ok_or(anyhow::anyhow!("failed to find branch {}", refname))?;

    branch_to_remote_branch_data(ctx, &branch)
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
    remotes: &[&str],
) -> Result<RemoteBranch> {
    let reference = branch.get();
    let name = Refname::try_from(branch)?;
    let given_name = reference.given_name(remotes)?;

    let commit = reference.peel_to_commit()?;
    let sha = commit.id();
    let is_remote = reference.is_remote();

    let author = commit.author().name().map(String::from);
    let last_commit_timestamp_ms = commit
        .time()
        .seconds()
        .try_into()
        .map(|t: u128| t * 1000)
        .ok();

    let upstream = if let Refname::Local(local_name) = &name {
        local_name.remote().cloned()
    } else {
        None
    };

    Ok(RemoteBranch {
        name,
        given_name,
        sha,
        upstream,
        is_remote,
        last_commit_timestamp_ms,
        last_commit_author: author,
    })
}

pub(crate) fn branch_to_remote_branch_data(
    ctx: &CommandContext,
    branch: &git2::Branch,
) -> Result<RemoteBranchData> {
    let name = Refname::try_from(branch)?;
    let base = default_target(&ctx.project().gb_dir())?.remote_head(ctx.repository())?;

    let sha = branch.get().target().context("failed to get target")?;
    let ahead = ctx.repository().log(sha, LogUntil::Commit(base), false)?;
    let behind = ctx.distance(base, sha)?;
    let fork_point = ahead.last().and_then(|c| c.parent(0).ok()).map(|c| c.id());

    let commits = ahead
        .into_iter()
        .map(|commit| commit_to_remote_commit(&commit))
        .collect::<Vec<_>>();

    let upstream = match &name {
        Refname::Local(local_name) => local_name.remote().cloned(),
        _ => None,
    };

    Ok(RemoteBranchData {
        name,
        sha,
        behind,
        upstream,
        fork_point,
        commits,
    })
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
