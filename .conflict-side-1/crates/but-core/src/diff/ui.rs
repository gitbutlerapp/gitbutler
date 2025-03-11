use crate::ui::{TreeChange, WorktreeChanges};
use gix::prelude::ObjectIdExt;
use std::path::PathBuf;

/// See [`super::worktree_changes()`].
pub fn worktree_changes_by_worktree_dir(worktree_dir: PathBuf) -> anyhow::Result<WorktreeChanges> {
    let repo = gix::open(worktree_dir)?;
    Ok(super::worktree_changes(&repo)?.into())
}

/// See [`super::commit_changes()`].
pub fn commit_changes_by_worktree_dir(
    worktree_dir: PathBuf,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Vec<TreeChange>> {
    let repo = gix::open(worktree_dir)?;
    let parent_id = commit_id
        .attach(&repo)
        .object()?
        .into_commit()
        .parent_ids()
        .map(|id| id.detach())
        .next();
    super::commit_changes(&repo, parent_id, commit_id)
        .map(|c| c.into_iter().map(Into::into).collect())
}
