use crate::{error::Error, from_json::HexHash};
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn list_actions(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    page: i64,
    page_size: i64,
) -> anyhow::Result<but_action::ActionListing, Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    but_action::list_actions(ctx, page, page_size).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn actions_revert_snapshot(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    snapshot: HexHash,
    description: &str,
) -> anyhow::Result<(), Error> {
    let project = projects.get(project_id)?;
    let mut ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let mut guard = ctx.project().exclusive_worktree_access();
    but_action::revert(&mut ctx, *snapshot, description, guard.write_permission())
        .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn handle_changes(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
) -> anyhow::Result<but_action::Outcome, Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    but_action::handle_changes(ctx, &change_summary, None, handler)
        .map_err(|e| Error::from(anyhow::anyhow!(e)))
}
