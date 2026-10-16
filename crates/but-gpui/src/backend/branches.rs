//! The branches outside the workspace: local ones it doesn't have applied, and ones only
//! a remote has, like a teammate's.

use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;
use but_branches::ListedStackStatus;

use super::{CommitId, ReviewState, open, review_state};

#[derive(Debug, Clone)]
pub struct OtherBranch {
    /// The local branch if there is one, else the remote-tracking one.
    pub ref_name: String,
    pub name: String,
    pub local: bool,
    pub author: Option<String>,
    /// When it last changed, in seconds since the epoch.
    pub time: Option<i64>,
    /// Commits it has that the target doesn't, when known.
    pub ahead: Option<usize>,
    pub tip: CommitId,
    pub review: Option<(u64, ReviewState)>,
}

/// Every branch the workspace doesn't have applied, newest first; not the target.
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
        .flat_map(|stack| stack.branches)
        .map(|listed| {
            let branch = listed.branch;
            OtherBranch {
                ref_name: branch.ref_name.to_string(),
                name: branch.display_name.to_str_lossy().into_owned(),
                local: branch.has_local,
                author: branch
                    .last_author
                    .map(|author| author.name.to_str_lossy().into_owned()),
                time: branch.updated_at_ms.map(|ms| ms / 1000),
                ahead: branch.commits_ahead_of_target,
                tip: CommitId(branch.tip),
                review: listed
                    .review
                    .map(|review| (review.number as u64, review_state(&review))),
            }
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
    let name = gix::refs::FullName::try_from(ref_name)?;
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
