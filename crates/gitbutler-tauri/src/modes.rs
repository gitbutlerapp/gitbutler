use but_api::commands::modes;
use but_core::ui::TreeChange;
use but_workspace::StackId;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::{EditModeMetadata, OperatingMode};
use gitbutler_project::ProjectId;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn operating_mode(project_id: ProjectId) -> Result<OperatingMode, Error> {
    modes::operating_mode(modes::OperatingModeParams { project_id })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn enter_edit_mode(
    project_id: ProjectId,
    commit_id: String,
    stack_id: StackId,
) -> Result<EditModeMetadata, Error> {
    modes::enter_edit_mode(modes::EnterEditModeParams {
        project_id,
        commit_id,
        stack_id,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn abort_edit_and_return_to_workspace(project_id: ProjectId) -> Result<(), Error> {
    modes::abort_edit_and_return_to_workspace(modes::AbortEditAndReturnToWorkspaceParams {
        project_id,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn save_edit_and_return_to_workspace(project_id: ProjectId) -> Result<(), Error> {
    modes::save_edit_and_return_to_workspace(modes::SaveEditAndReturnToWorkspaceParams {
        project_id,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn edit_initial_index_state(
    project_id: ProjectId,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>, Error> {
    modes::edit_initial_index_state(modes::EditInitialIndexStateParams { project_id })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn edit_changes_from_initial(project_id: ProjectId) -> Result<Vec<TreeChange>, Error> {
    modes::edit_changes_from_initial(modes::EditChangesFromInitialParams { project_id })
}
