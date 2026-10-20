//! The branches outside the workspace: local ones it doesn't have applied, and ones only
//! a remote has, like a teammate's.

use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;
use but_branches::ListedStackStatus;

use super::ops::full_name;
use super::reviews::from_cache;
use super::{CommitId, Review, open, title};

#[derive(Debug, Clone)]
pub struct OtherBranch {
    /// The local branch if there is one, else the remote-tracking one.
    pub ref_name: String,
    pub name: String,
    pub local: bool,
    pub author: Option<String>,
    /// Its newest commit's author's, for their picture.
    pub email: Option<String>,
    /// When it last changed, in seconds since the epoch.
    pub time: Option<i64>,
    /// Commits it has that the target doesn't, when known.
    pub ahead: Option<usize>,
    /// Commits of its own, down to the branch it's stacked on or where it forked, when known.
    pub own_commits: Option<usize>,
    /// Which of the listed stacks it's in, so a stack's branches stay together.
    pub stack: usize,
    pub tip: CommitId,
    /// Its pull request, without CI checks: the app asks the forge about those only for the
    /// workspace's branches, so the cache's would be stale.
    pub review: Option<Review>,
}

/// Every branch the workspace doesn't have applied, newest first, a stack's branches together
/// from the top; not the target.
pub fn other_branches(path: &Path) -> Result<Vec<OtherBranch>> {
    let ctx = open(path)?;
    Ok(but_api::branch::branch_list(&ctx)?
        .into_iter()
        .filter(|stack| {
            matches!(
                stack.status,
                ListedStackStatus::Unapplied | ListedStackStatus::Standalone
            )
        })
        .enumerate()
        .flat_map(|(stack, listed)| listed.branches.into_iter().map(move |b| (stack, b)))
        .map(|(stack, listed)| {
            let branch = listed.branch;
            OtherBranch {
                ref_name: branch.ref_name.to_string(),
                name: branch.display_name.to_str_lossy().into_owned(),
                local: branch.has_local,
                author: (branch.last_author.as_ref())
                    .map(|author| author.name.to_str_lossy().into_owned()),
                email: (branch.last_author.as_ref())
                    .map(|author| author.email.to_str_lossy().into_owned()),
                time: branch.updated_at_ms.map(|ms| ms / 1000),
                ahead: branch.commits_ahead_of_target,
                own_commits: branch.commit_count,
                stack,
                tip: CommitId(branch.tip),
                review: listed.review.as_ref().map(from_cache),
            }
        })
        .collect())
}

/// A commit `ref_name` has that the target doesn't.
#[derive(Debug, Clone)]
pub struct BranchCommit {
    pub id: CommitId,
    pub title: String,
    /// Seconds since the epoch.
    pub time: i64,
}

/// The commits `ref_name` has that the target doesn't, newest first.
pub fn branch_commits(path: &Path, ref_name: &str) -> Result<Vec<BranchCommit>> {
    let ctx = open(path)?;
    let name = full_name(ref_name)?;
    let remote = but_core::extract_remote_name_and_short_name(
        name.as_ref(),
        &ctx.repo.get()?.remote_names(),
    );
    let (short, remote) = match remote {
        Some((remote, short)) => (short.to_string(), Some(remote)),
        None => (name.shorten().to_string(), None),
    };
    let details = but_api::legacy::workspace::branch_details(&ctx, short, remote)?;
    Ok(details
        .commits
        .into_iter()
        .map(|commit| BranchCommit {
            id: CommitId(commit.id),
            title: title(&commit.message),
            time: (commit.committed_at / 1000) as i64,
        })
        .collect())
}

/// How applying a branch went.
pub enum ApplyOutcome {
    Applied,
    AlreadyApplied,
    /// It conflicts with these applied stacks, so nothing changed.
    Conflicts(Vec<String>),
}

/// Apply `ref_name` to the workspace. A remote branch gets a local branch to apply.
pub fn apply(path: &Path, ref_name: &str) -> Result<ApplyOutcome> {
    use but_workspace::branch::apply::OutcomeStatus;
    let mut ctx = open(path)?;
    let name = full_name(ref_name)?;
    let outcome = but_api::branch::apply(&mut ctx, name.as_ref())?;
    Ok(match outcome.status {
        OutcomeStatus::Applied => ApplyOutcome::Applied,
        OutcomeStatus::AlreadyApplied => ApplyOutcome::AlreadyApplied,
        OutcomeStatus::ConflictAborted => ApplyOutcome::Conflicts(
            outcome
                .conflicting_stacks
                .iter()
                .map(|stack| stack.ref_name.shorten().to_string())
                .collect(),
        ),
    })
}
