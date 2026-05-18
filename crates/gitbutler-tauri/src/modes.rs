use but_api::{json, legacy};
use but_ctx::ProjectHandleOrLegacyProjectId;
use tauri::Window;
use tracing::instrument;

use crate::git_operation_progress::GitOperationProgressEmitter;

#[tauri::command]
#[instrument(skip(window), err(Debug))]
#[allow(non_snake_case)]
pub fn save_edit_and_return_to_workspace(
    window: Window,
    projectId: ProjectHandleOrLegacyProjectId,
) -> Result<(), json::Error> {
    let progress = GitOperationProgressEmitter::new(&window, &projectId, "returnToWorkspace");
    progress.phase(
        "prepare",
        "Preparing workspace checkout",
        Some("Git LFS hydration is deferred for this operation.".to_string()),
    );
    let mut ctx = but_ctx::Context::try_from(projectId)?;
    let _lfs_scope = but_core::lfs::LfsFastOperationScope::new();
    progress.phase("snapshot", "Capturing edit-mode changes", None);
    let result = legacy::modes::save_edit_and_return_to_workspace(&mut ctx);
    match result {
        Ok(()) => {
            progress.phase("complete", "Workspace checkout complete", None);
            Ok(())
        }
        Err(err) => {
            progress.phase("failed", "Workspace checkout failed", Some(err.to_string()));
            Err(err.into())
        }
    }
}

#[tauri::command]
#[instrument(skip(window), err(Debug))]
#[allow(non_snake_case)]
pub fn abort_edit_and_return_to_workspace(
    window: Window,
    projectId: ProjectHandleOrLegacyProjectId,
    force: bool,
) -> Result<(), json::Error> {
    let progress = GitOperationProgressEmitter::new(&window, &projectId, "returnToWorkspace");
    progress.phase(
        "prepare",
        "Preparing workspace checkout",
        Some("Git LFS hydration is deferred for this operation.".to_string()),
    );
    let mut ctx = but_ctx::Context::try_from(projectId)?;
    let _lfs_scope = but_core::lfs::LfsFastOperationScope::new();
    progress.phase("checkout", "Restoring workspace files", None);
    let result = legacy::modes::abort_edit_and_return_to_workspace(&mut ctx, force);
    match result {
        Ok(()) => {
            progress.phase("complete", "Workspace checkout complete", None);
            Ok(())
        }
        Err(err) => {
            progress.phase("failed", "Workspace checkout failed", Some(err.to_string()));
            Err(err.into())
        }
    }
}
