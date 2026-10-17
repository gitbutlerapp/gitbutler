//! Edit mode: one commit checked out in the worktree, conflict markers and all, for any
//! editor or agent to work on, then saved back into the commit or given up.

use std::path::Path;

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_core::{
    ChangeState, IgnoredWorktreeChange, IgnoredWorktreeTreeChangeStatus, TreeChange, TreeStatus,
};
use but_ctx::Context;
use gitbutler_operating_modes::OperatingMode;

use super::{CommitId, open};

/// The commit being edited, and how far along its conflicts are.
#[derive(Debug, Clone)]
pub struct Editing {
    pub commit: CommitId,
    pub title: String,
    pub files: Vec<EditedFile>,
    /// Paths changed since editing started.
    pub changed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EditedFile {
    pub path: String,
    /// `None` for a file without conflicts, else whether its markers are gone.
    pub resolved: Option<bool>,
}

/// The commit `ctx` is editing, if it's in edit mode.
pub(super) fn editing(ctx: &Context) -> Result<Option<Editing>> {
    let OperatingMode::Edit(meta) = but_api::legacy::modes::operating_mode(ctx)?.operating_mode
    else {
        return Ok(None);
    };
    let repo = ctx.repo.get()?;
    let workdir = repo.workdir().context("the repository has no worktree")?;
    let message = repo.find_commit(meta.commit_oid)?.message_raw()?.to_owned();
    let message = but_core::commit::strip_conflict_markers(message.as_ref());
    let title = message.lines().next().unwrap_or_default();
    let title = title.to_str_lossy().trim().to_owned();
    let files: Vec<EditedFile> = but_api::legacy::modes::edit_initial_index_state(ctx)?
        .into_iter()
        .map(|(change, presence)| {
            let path = change.path.to_str_lossy().into_owned();
            let resolved = presence.map(|_| {
                std::fs::read(workdir.join(&path)).map_or(true, |text| !has_markers(&text))
            });
            EditedFile { path, resolved }
        })
        .collect();
    // A file with its markers still in differs from the commit as stored, which holds
    // the conflict's automatic resolution; it isn't news until its markers are gone.
    let changed = but_api::legacy::modes::edit_changes_from_initial(ctx)?
        .into_iter()
        .map(|change| change.path.to_str_lossy().into_owned())
        .filter(|path| {
            !(files.iter()).any(|f: &EditedFile| f.path == *path && f.resolved == Some(false))
        })
        .collect();
    Ok(Some(Editing {
        commit: CommitId(meta.commit_oid),
        title,
        files,
        changed,
    }))
}

/// The files Git's index still has conflicted, as changes from `HEAD` to what's on disk:
/// editing a commit leaves its conflicts there, and status passes over them.
pub(super) fn conflicted_changes(
    repo: &gix::Repository,
    ignored: &[IgnoredWorktreeChange],
) -> Result<Vec<TreeChange>> {
    let conflicted = |change: &&IgnoredWorktreeChange| {
        change.status == IgnoredWorktreeTreeChangeStatus::Conflict
    };
    // Status runs on every change to the worktree; most of the time nothing's conflicted.
    if !ignored.iter().any(|change| conflicted(&change)) {
        return Ok(Vec::new());
    }
    let head = repo.head_tree()?;
    let on_disk = |kind| ChangeState {
        id: gix::ObjectId::null(repo.object_hash()),
        kind,
    };
    let mut changes = Vec::new();
    for change in ignored.iter().filter(conflicted) {
        let path = gix::path::from_bstr(change.path.as_bstr())?;
        let status = match head.lookup_entry_by_path(path)? {
            Some(entry) => {
                let kind = entry.mode().kind();
                TreeStatus::Modification {
                    previous_state: ChangeState {
                        id: entry.object_id(),
                        kind,
                    },
                    state: on_disk(kind),
                    flags: None,
                }
            }
            None => TreeStatus::Addition {
                state: on_disk(gix::object::tree::EntryKind::Blob),
                is_untracked: false,
            },
        };
        changes.push(TreeChange {
            path: change.path.clone(),
            status,
        });
    }
    Ok(changes)
}

/// Whether `text` still has a conflict between markers, as Git writes them.
fn has_markers(text: &[u8]) -> bool {
    let mut lines = text.lines();
    lines.any(|line| line.starts_with(b"<<<<<<<")) && lines.any(|line| line.starts_with(b">>>>>>>"))
}

/// Check `commit` out to edit it.
pub fn enter(path: &Path, commit: CommitId) -> Result<()> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let stack_id = info
        .stacks
        .iter()
        .find(|stack| (stack.segments.iter()).any(|s| s.commits.iter().any(|c| c.id == commit.0)))
        .and_then(|stack| stack.id)
        .context("That commit isn't in the workspace any more")?;
    but_api::legacy::modes::enter_edit_mode(&mut ctx, commit.0, stack_id)?;
    Ok(())
}

/// Rewrite the edited commit from the worktree, rebase what sat on it, and go back to the
/// workspace.
pub fn save(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::legacy::modes::save_edit_and_return_to_workspace(&mut ctx)
}

/// Go back to the workspace as it was, dropping the edits.
pub fn cancel(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::legacy::modes::abort_edit_and_return_to_workspace(&mut ctx, true)
}

#[cfg(test)]
mod tests {
    use super::has_markers;

    #[test]
    fn finds_conflict_markers() {
        assert!(
            has_markers(b"a\n<<<<<<< ours\nb\n=======\nc\n>>>>>>> theirs\n"),
            "a whole conflict"
        );
        assert!(!has_markers(b"a\nb\n"), "no markers");
        assert!(
            !has_markers(b">>>>>>> theirs\n<<<<<<< ours\n"),
            "an end before a start isn't a conflict"
        );
    }
}
