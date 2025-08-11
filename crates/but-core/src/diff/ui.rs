use crate::{
    Commit,
    commit::ConflictEntries,
    ui::{TreeChanges, WorktreeChanges},
};
use gix::prelude::ObjectIdExt;
use std::path::PathBuf;

/// See [`super::worktree_changes()`].
pub fn worktree_changes_by_worktree_dir(worktree_dir: PathBuf) -> anyhow::Result<WorktreeChanges> {
    let repo = gix::open(worktree_dir)?;
    Ok(super::worktree_changes(&repo)?.into())
}

/// See [`super::commit_changes()`].
pub fn commit_changes_by_worktree_dir(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<TreeChanges> {
    let parent_id = commit_id
        .attach(repo)
        .object()?
        .into_commit()
        .parent_ids()
        .map(|id| id.detach())
        .next();
    let (changes, stats) = super::tree_changes(repo, parent_id, commit_id)
        .map(|(c, s)| (c.into_iter().map(Into::into).collect(), s.into()))?;
    Ok(TreeChanges { changes, stats })
}

/// If the commit is conflicted, it will return the entries that are in fact
/// conflicted.
pub fn conflicted_changes(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Option<ConflictEntries>> {
    let commit = Commit::from_id(commit_id.attach(repo))?;

    commit.conflict_entries()
}

/// See [`super::tree_changes()`].
pub fn changes_in_range(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    base_commit: gix::ObjectId,
) -> anyhow::Result<TreeChanges> {
    let (changes, stats) = super::tree_changes(repo, Some(base_commit), commit_id)
        .map(|(c, s)| (c.into_iter().map(Into::into).collect(), s.into()))?;
    Ok(TreeChanges { changes, stats })
}
