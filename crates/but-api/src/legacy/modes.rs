//! In place of commands.rs
use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_core::{ref_metadata::StackId, ui::TreeChange};
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::{EditModeMetadata, OperatingMode};
use tracing::instrument;

use crate::json::Error;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadAndMode {
    pub head: Option<String>,
    pub operating_mode: Option<OperatingMode>,
}

#[but_api]
#[instrument(err(Debug))]
pub fn operating_mode(ctx: &Context) -> Result<HeadAndMode, Error> {
    let head = match ctx.git2_repo.get()?.head() {
        Ok(head_ref) => head_ref.shorthand().map(|s| s.to_string()),
        Err(_) => None,
    };

    Ok(HeadAndMode {
        head,
        operating_mode: Some(gitbutler_operating_modes::operating_mode(ctx)),
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadSha {
    head_sha: String,
}

#[but_api]
#[instrument(err(Debug))]
pub fn head_sha(ctx: &but_ctx::Context) -> Result<HeadSha, Error> {
    let git2_repo = ctx.git2_repo.get()?;
    let head_ref = git2_repo.head().context("failed to get head")?;
    let head_sha = head_ref
        .peel_to_commit()
        .context("failed to get head commit")?
        .id()
        .to_string();

    Ok(HeadSha { head_sha })
}

#[but_api]
#[instrument(err(Debug))]
pub fn enter_edit_mode(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    stack_id: StackId,
) -> Result<EditModeMetadata> {
    gitbutler_edit_mode::commands::enter_edit_mode(ctx, commit_id.to_git2(), stack_id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn abort_edit_and_return_to_workspace(ctx: &mut but_ctx::Context) -> Result<()> {
    gitbutler_edit_mode::commands::abort_and_return_to_workspace(ctx)?;

    Ok(())
}

// GUI-facing API that returns () for serialization compatibility
#[but_api]
#[instrument(err(Debug))]
pub fn save_edit_and_return_to_workspace(ctx: &mut but_ctx::Context) -> Result<()> {
    gitbutler_edit_mode::commands::save_and_return_to_workspace(ctx)?;

    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn save_edit_and_return_to_workspace_with_output(ctx: &mut but_ctx::Context) -> Result<()> {
    gitbutler_edit_mode::commands::save_and_return_to_workspace(ctx)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn edit_initial_index_state(
    ctx: &mut but_ctx::Context,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    gitbutler_edit_mode::commands::starting_index_state(ctx)
}

#[but_api]
#[instrument(err(Debug))]
pub fn edit_changes_from_initial(ctx: &mut but_ctx::Context) -> Result<Vec<TreeChange>> {
    gitbutler_edit_mode::commands::changes_from_initial(ctx)
}
