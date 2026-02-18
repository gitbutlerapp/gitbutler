//! In place of commands.rs
use anyhow::Result;
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
    let repo = ctx.repo.get()?;
    let head = repo.head();
    let head_ref_short = match head.as_ref().map(|head| head.referent_name()) {
        Ok(Some(head_ref)) => Some(head_ref.shorten().to_string()),
        _ => None,
    };

    Ok(HeadAndMode {
        head: head_ref_short,
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
    let repo = ctx.repo.get()?;
    let mut head_ref = repo.head().map_err(anyhow::Error::from)?;
    let head_sha = head_ref.peel_to_commit().map_err(anyhow::Error::from)?.id.to_string();
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
pub fn abort_edit_and_return_to_workspace(ctx: &mut but_ctx::Context, force: bool) -> Result<()> {
    gitbutler_edit_mode::commands::abort_and_return_to_workspace(ctx, force)?;

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
