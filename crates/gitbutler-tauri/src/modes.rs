use but_api::{commands::modes, IpcContext};
use but_core::ui::TreeChange;
use but_workspace::StackId;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::{EditModeMetadata, OperatingMode};
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn operating_mode(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<OperatingMode, Error> {
    modes::operating_mode(&ipc_ctx, modes::OperatingModeParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn enter_edit_mode(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    commit_id: String,
    stack_id: StackId,
) -> Result<EditModeMetadata, Error> {
    modes::enter_edit_mode(
        &ipc_ctx,
        modes::EnterEditModeParams {
            project_id,
            commit_id,
            stack_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn abort_edit_and_return_to_workspace(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<(), Error> {
    modes::abort_edit_and_return_to_workspace(
        &ipc_ctx,
        modes::AbortEditAndReturnToWorkspaceParams { project_id },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn save_edit_and_return_to_workspace(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<(), Error> {
    modes::save_edit_and_return_to_workspace(
        &ipc_ctx,
        modes::SaveEditAndReturnToWorkspaceParams { project_id },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn edit_initial_index_state(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>, Error> {
    modes::edit_initial_index_state(&ipc_ctx, modes::EditInitialIndexStateParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn edit_changes_from_initial(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<Vec<TreeChange>, Error> {
    modes::edit_changes_from_initial(&ipc_ctx, modes::EditChangesFromInitialParams { project_id })
}
