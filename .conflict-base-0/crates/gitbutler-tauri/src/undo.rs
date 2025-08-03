use but_api::{commands::undo, IpcContext};
use gitbutler_oplog::entry::OperationKind;
use gitbutler_oplog::entry::Snapshot;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn list_snapshots(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
    exclude_kind: Option<Vec<OperationKind>>,
) -> Result<Vec<Snapshot>, Error> {
    undo::list_snapshots(
        &ipc_ctx,
        undo::ListSnapshotsParams {
            project_id,
            limit,
            sha,
            exclude_kind,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn restore_snapshot(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    sha: String,
) -> Result<(), Error> {
    undo::restore_snapshot(&ipc_ctx, undo::RestoreSnapshotParams { project_id, sha })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn snapshot_diff(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    sha: String,
) -> Result<Vec<but_core::ui::TreeChange>, Error> {
    undo::snapshot_diff(&ipc_ctx, undo::SnapshotDiffParams { project_id, sha })
}
