use crate::ui::{TreeChange, WorktreeChanges};
use std::path::PathBuf;

/// See [`super::worktree_changes()`].
pub fn worktree_changes_by_worktree_dir(worktree_dir: PathBuf) -> anyhow::Result<WorktreeChanges> {
    let repo = gix::open(worktree_dir)?;
    Ok(super::worktree_changes(&repo)?.into())
}

/// See [`super::commit_changes()`].
pub fn commit_changes_by_worktree_dir(
    worktree_dir: PathBuf,
    old_commit_id: Option<gix::ObjectId>,
    new_commit_id: gix::ObjectId,
) -> anyhow::Result<Vec<TreeChange>> {
    let repo = gix::open(worktree_dir)?;
    super::commit_changes(&repo, old_commit_id, new_commit_id)
        .map(|c| c.into_iter().map(Into::into).collect())
}
