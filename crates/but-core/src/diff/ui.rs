// TODO: all of these should go away.
use gix::prelude::ObjectIdExt;

use std::collections::BTreeMap;

use bstr::ByteSlice;

use crate::{
    Commit, IgnoredWorktreeTreeChangeStatus,
    commit::ConflictEntries,
    ui::{TreeChanges, WorktreeChanges},
};

/// See [`super::worktree_changes()`].
pub fn worktree_changes(repo: &gix::Repository) -> anyhow::Result<WorktreeChanges> {
    Ok(super::worktree_changes(repo)?.into())
}

/// Modification times for `changes`, one `symlink_metadata` per path, keyed as
/// [`WorktreeChanges::modification_times`] describes. Conflicted ignored changes
/// are statted too — a conflict is still a file whose recency the user can ask
/// about. Paths that cannot be statted, deletions foremost, are absent.
///
/// Separate from [`worktree_changes()`] so only callers wanting the times pay
/// for the stat pass; most take just the changes.
pub fn modification_times(
    repo: &gix::Repository,
    changes: &WorktreeChanges,
) -> BTreeMap<String, u64> {
    let Some(workdir) = repo.workdir() else {
        return BTreeMap::new();
    };
    let conflicts = changes
        .ignored_changes
        .iter()
        .filter(|ignored| matches!(ignored.status, IgnoredWorktreeTreeChangeStatus::Conflict))
        .map(|ignored| ignored.path.as_bstr());
    changes
        .changes
        .iter()
        .map(|change| change.path_bytes.as_bstr())
        .chain(conflicts)
        .filter_map(|path_bytes| {
            let path = workdir.join(gix::path::from_bstr(path_bytes));
            let modified = path.symlink_metadata().ok()?.modified().ok()?;
            let millis = modified
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_millis();
            // The same lossy conversion the paths serialize with, so the key
            // always matches the change the frontend sees.
            Some((
                path_bytes.to_str_lossy().into_owned(),
                u64::try_from(millis).ok()?,
            ))
        })
        .collect()
}

/// See [`super::tree_changes_with_line_stats()`].
pub fn commit_changes_with_line_stats_by_worktree_dir(
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
    let (changes, stats) = super::tree_changes_with_line_stats(repo, parent_id, commit_id)
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
pub fn changes_with_line_stats_in_range(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    base_commit: gix::ObjectId,
) -> anyhow::Result<TreeChanges> {
    let (changes, stats) = super::tree_changes_with_line_stats(repo, Some(base_commit), commit_id)
        .map(|(c, s)| (c.into_iter().map(Into::into).collect(), s.into()))?;
    Ok(TreeChanges { changes, stats })
}
