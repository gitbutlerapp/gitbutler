//! Edit mode, as `but resolve <commit>` starts it: one commit checked out in the worktree,
//! conflict markers and all, until `but resolve finish` or `but resolve cancel`.

use anyhow::Result;
use but_ctx::Context;
use gitbutler_operating_modes::OperatingMode;

use super::{CommitId, title};

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
