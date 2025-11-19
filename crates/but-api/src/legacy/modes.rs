//! In place of commands.rs
use anyhow::{Context as _, Result};
use but_api_macros::api_cmd_tauri;
use but_core::{ref_metadata::StackId, ui::TreeChange};
use but_ctx::Context;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::{EditModeMetadata, OperatingMode};
use gitbutler_project::ProjectId;
use tracing::instrument;

use crate::json::Error;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadAndMode {
    pub head: Option<String>,
    pub operating_mode: Option<OperatingMode>,
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn operating_mode(project_id: ProjectId) -> Result<HeadAndMode, Error> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let head = match ctx.git2_repo.get()?.head() {
        Ok(head_ref) => head_ref.shorthand().map(|s| s.to_string()),
        Err(_) => None,
    };

    Ok(HeadAndMode {
        head,
        operating_mode: Some(gitbutler_operating_modes::operating_mode(&ctx)),
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadSha {
    head_sha: String,
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn head_sha(project_id: ProjectId) -> Result<HeadSha, Error> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let git2_repo = ctx.git2_repo.get()?;
    let head_ref = git2_repo.head().context("failed to get head")?;
    let head_sha = head_ref
        .peel_to_commit()
        .context("failed to get head commit")?
        .id()
        .to_string();

    Ok(HeadSha { head_sha })
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn enter_edit_mode(
    project_id: ProjectId,
    commit_id: String,
    stack_id: StackId,
) -> Result<EditModeMetadata> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit = git2::Oid::from_str(&commit_id).context("Failed to parse commit oid")?;

    gitbutler_edit_mode::commands::enter_edit_mode(&ctx, commit, stack_id)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn abort_edit_and_return_to_workspace(project_id: ProjectId) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;

    gitbutler_edit_mode::commands::abort_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn save_edit_and_return_to_workspace(project_id: ProjectId) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;

    gitbutler_edit_mode::commands::save_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn edit_initial_index_state(
    project_id: ProjectId,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;

    gitbutler_edit_mode::commands::starting_index_state(&ctx)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn edit_changes_from_initial(project_id: ProjectId) -> Result<Vec<TreeChange>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;

    gitbutler_edit_mode::commands::changes_from_initial(&ctx)
}
