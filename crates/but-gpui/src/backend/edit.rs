//! Edit mode, as `but resolve <commit>` starts it: one commit checked out in the worktree,
//! conflict markers and all, until it's finished or cancelled, here or with `but resolve`.

use std::path::Path;

use anyhow::Result;
use but_ctx::Context;
use gitbutler_operating_modes::OperatingMode;

use super::{CommitId, open, title};

/// The commit being edited.
#[derive(Debug, Clone)]
pub struct Editing {
    pub commit: CommitId,
    pub title: String,
}

/// The commit `ctx` is editing, if it's in edit mode.
pub(super) fn editing(ctx: &Context) -> Result<Option<Editing>> {
    let OperatingMode::Edit(meta) = but_api::legacy::modes::operating_mode(ctx)?.operating_mode
    else {
        return Ok(None);
    };
    let repo = ctx.repo.get()?;
    let message = repo.find_commit(meta.commit_oid)?.message_raw()?.to_owned();
    Ok(Some(Editing {
        commit: CommitId(meta.commit_oid),
        title: title(&but_core::commit::strip_conflict_markers(message.as_ref())),
    }))
}

/// Commit what's in the worktree in place of the commit being edited, rebasing what's above
/// it, and go back to the workspace, as `but resolve finish` does.
pub fn finish(path: &Path) -> Result<()> {
    but_api::legacy::modes::save_edit_and_return_to_workspace(&mut open(path)?)
}

/// Go back to the workspace, leaving the commit as it was. Unless `force`, only when nothing
/// changed since it was checked out, as otherwise that's lost; returns whether it went back.
pub fn cancel(path: &Path, force: bool) -> Result<bool> {
    let mut ctx = open(path)?;
    if !force && !but_api::legacy::modes::edit_changes_from_initial(&ctx)?.is_empty() {
        return Ok(false);
    }
    but_api::legacy::modes::abort_edit_and_return_to_workspace(&mut ctx, true)?;
    Ok(true)
}
