use crate::ui::{TreeChanges, WorktreeChanges};
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
) -> anyhow::Result<TreeChanges> {
    let repo = gix::open(worktree_dir)?;
    let parent_id = commit_id
        .attach(&repo)
        .object()?
        .into_commit()
        .parent_ids()
        .map(|id| id.detach())
        .next();
    let (changes, stats) = super::commit_changes(&repo, parent_id, commit_id)
        .map(|(c, s)| (c.into_iter().map(Into::into).collect(), s.into()))?;
    Ok(TreeChanges { changes, stats })
}

/// See [`super::commit_changes()`].
pub fn changes_in_commit_range(
    worktree_dir: PathBuf,
    commit_id: gix::ObjectId,
    base: gix::ObjectId,
) -> anyhow::Result<TreeChanges> {
    let repo = gix::open(worktree_dir)?;
    let (changes, stats) = super::commit_changes(&repo, Some(base), commit_id)
        .map(|(c, s)| (c.into_iter().map(Into::into).collect(), s.into()))?;
    Ok(TreeChanges { changes, stats })
}
