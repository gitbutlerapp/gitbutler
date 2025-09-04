use but_api::commands::undo;
use gitbutler_oplog::entry::OperationKind;
use gitbutler_oplog::entry::Snapshot;
use gitbutler_project::ProjectId;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn list_snapshots(
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
    exclude_kind: Option<Vec<OperationKind>>,
) -> Result<Vec<Snapshot>, Error> {
    undo::list_snapshots(undo::ListSnapshotsParams {
        project_id,
        limit,
        sha,
        exclude_kind,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn restore_snapshot(project_id: ProjectId, sha: String) -> Result<(), Error> {
    undo::restore_snapshot(undo::RestoreSnapshotParams { project_id, sha })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn snapshot_diff(
    project_id: ProjectId,
    sha: String,
) -> Result<Vec<but_core::ui::TreeChange>, Error> {
    undo::snapshot_diff(undo::SnapshotDiffParams { project_id, sha })
}
