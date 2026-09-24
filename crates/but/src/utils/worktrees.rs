use anyhow::Context as _;
use bstr::BStr;

use crate::{
    CliId, CliResult,
    args::atoms::CliIdArg,
    bad_input,
    id::AnonymousSegmentId,
    utils::{change_source::ChangeSourceId, targeting::Side},
};

/// The branch checked out in the linked worktree `name`, which is where a commit made from that
/// checkout goes and where a commit moved onto its lane lands.
///
/// A detached worktree has no branch to move, so it is refused rather than silently targeting
/// somewhere else. The same goes for a workspace ref: such worktrees never render a heading,
/// but the checkout can change between render and confirm, and a workspace ref must never
/// receive a commit meant for a branch.
pub(crate) fn worktree_branch(
    repo: &gix::Repository,
    name: &BStr,
) -> anyhow::Result<gix::refs::FullName> {
    let worktree_repo = but_workspace::worktrees::open_worktree_repo(repo, name)?;
    let branch = worktree_repo.head_name()?.with_context(|| {
        format!("Worktree {name} has a detached HEAD, so there is no branch to commit to")
    })?;
    anyhow::ensure!(
        !but_core::is_workspace_ref_name(branch.as_ref()),
        "Worktree {name} has a workspace ref checked out, so there is no branch to commit to"
    );
    Ok(branch)
}

pub(crate) fn is_worktree_top(
    repo: &gix::Repository,
    name: &BStr,
    segment: &CliId,
) -> anyhow::Result<bool> {
    match segment {
        CliId::Branch(branch) => {
            let branch = gix::refs::Category::LocalBranch.to_full_name(branch.name.as_str())?;
            let worktree_repo = but_workspace::worktrees::open_worktree_repo(repo, name)?;
            Ok(worktree_repo.head_name()? == Some(branch))
        }
        CliId::AnonymousSegment(AnonymousSegmentId {
            anchor_commit_id: Some(anchor),
            ..
        }) => {
            let worktree_repo = but_workspace::worktrees::open_worktree_repo(repo, name)?;
            Ok(
                worktree_repo.head_name()?.is_none()
                    && worktree_repo.head_id()?.detach() == *anchor,
            )
        }
        CliId::AnonymousSegment(AnonymousSegmentId {
            anchor_commit_id: None,
            ..
        })
        | CliId::Commit { .. }
        | CliId::CommittedFile { .. }
        | CliId::CommittedHunk(..)
        | CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::Uncommitted { .. }
        | CliId::WorktreeUncommitted { .. }
        | CliId::Stack { .. } => Ok(false),
    }
}

/// The tip an `--above`/`--below` argument naming the worktree `name` targets.
///
/// Below the heading is the top of its lane, so the tip of the branch checked out there. Above
/// it is the worktree's uncommitted area, which cannot hold a commit; `target_arg` is the
/// user's spelling of the worktree, for attributing that refusal.
pub(crate) fn worktree_tip_target(
    repo: &gix::Repository,
    name: &BStr,
    side: Side,
    target_arg: &CliIdArg,
) -> CliResult<gix::refs::FullName> {
    match side {
        // A detached or otherwise branchless worktree is bad input naming this
        // target, not an internal failure.
        Side::Below => worktree_branch(repo, name).map_err(|err| bad_input(err.to_string()).into()),
        Side::Above => Err(bad_input("Cannot place a commit above a worktree")
            .arg_name("--above")
            .arg_value(target_arg.to_string())
            .hint("Use `--below` to target the tip of the worktree's branch")
            .into()),
    }
}

/// The worktree an uncommit of `commit` lands in: the linked worktree that owns it, or the main
/// worktree for a workspace commit.
pub(crate) fn commit_owner(
    head_info: &but_workspace::RefInfo,
    commit: gix::ObjectId,
) -> ChangeSourceId {
    head_info
        .worktrees
        .iter()
        .find(|worktree| worktree.commits().any(|owned| owned.id == commit))
        .map_or(ChangeSourceId::Head, |worktree| {
            ChangeSourceId::Worktree(worktree.name.clone())
        })
}
