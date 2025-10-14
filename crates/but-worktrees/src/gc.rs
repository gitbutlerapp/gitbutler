use crate::{Worktree, WorktreeHealthStatus, WorktreeSource};
use anyhow::Result;
use bstr::BStr;
use but_workspace::ui::StackHeadInfo;

pub fn get_health(
    repo: &gix::Repository,
    worktree: &Worktree,
    heads: &[StackHeadInfo],
) -> Result<WorktreeHealthStatus> {
    if !heads.iter().any(|h| match &worktree.source {
        WorktreeSource::Branch(b) => h.name == BStr::new(&b),
    }) {
        return Ok(WorktreeHealthStatus::WorkspaceBranchMissing);
    };

    let git_worktrees = repo.worktrees()?;
    let Some(git_worktree) = git_worktrees
        .iter()
        .find(|w| w.base().map(|b| b == worktree.path).unwrap_or(false))
    else {
        return Ok(WorktreeHealthStatus::WorktreeMissing);
    };

    if repo.try_find_reference(&worktree.reference)?.is_none() {
        return Ok(WorktreeHealthStatus::BranchMissing);
    };
    let worktree_repo = git_worktree.clone().into_repo()?;

    if !worktree_repo
        .head()?
        .referent_name()
        .map(|n| n.as_bstr() == worktree.reference)
        .unwrap_or(false)
    {
        return Ok(WorktreeHealthStatus::BranchNotCheckedOut);
    }

    Ok(WorktreeHealthStatus::Normal)
}
