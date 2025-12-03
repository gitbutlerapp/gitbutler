//! GitButler has an operation log (oplog) that records significant actions taken within a project.
//! The oplog records snapshots of the project state at the time when an operation was initiated.
//!
//! Using the oplog, it is possible to restore the project to a previous state by reverting to one of these snapshots.
//! This includes the state of the working directory as well as commmit history and references.
//!
//! This module provides commands to interact with the oplog, including listing snapshots, creating new snapshots,
//! restoring to a snapshot, and viewing differences between snapshots.
//!
//! An example usage:
//!   - A user squashes two commits together.
//!   - Listing the shapshot will show a new snapshot with the operation kind `SquashCommit`.
//!   - Restoring to the snapshot will revert the project to the state before the squash.
//!   - A new snapshot is created for the restore operation.
//!
//! Depending on the snapshot operation kind, there may be a payload (body) with additional details about the operation (e.g. commit message).
//! Refer to `gitbutler_oplog::entry::Snapshot` and `gitbutler_oplog::entry::SnapshotDetails` for the metadata stored.
//!
use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_ctx::Context;
use but_oxidize::OidExt;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, Snapshot, SnapshotDetails},
};
use gitbutler_project::ProjectId;
use tracing::instrument;

/// List snapshots in the oplog.
///
/// - `project_id`: The ID of the project to list snapshots for.
/// - `limit`: Maximum number of snapshots to return.
/// - `sha`: Optional SHA to filter snapshots starting from a specific commit.
/// - `exclude_kind`: Optional list of operation kinds to exclude from the results.
///
/// Returns a vector of `Snapshot` entries.
///
/// # Errors
/// Returns an error if the project cannot be found or if there is an issue accessing the oplog.
#[but_api]
#[instrument(err(Debug))]
pub fn list_snapshots(
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
    exclude_kind: Option<Vec<OperationKind>>,
) -> Result<Vec<Snapshot>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let snapshots = ctx.list_snapshots(
        limit,
        sha.map(|hex| hex.parse().map_err(anyhow::Error::from))
            .transpose()?,
        exclude_kind.unwrap_or_default(),
    )?;
    Ok(snapshots)
}

/// Gets a specific snapshot by its commit SHA.
///
/// - `project_id`: The ID of the project to get the snapshot for.
/// - `sha`: The SHA of the snapshot to retrieve.
///
/// Returns the `Snapshot` corresponding to the provided SHA.
///
/// # Errors
/// Returns an error if the project cannot be found, if the snapshot SHA is invalid, or if the underlying commit is not a valid snapshot commit
#[but_api]
#[instrument(err(Debug))]
pub fn get_snapshot(project_id: ProjectId, sha: String) -> Result<Snapshot> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let snapshot = ctx.get_snapshot(sha.parse().map_err(anyhow::Error::from)?)?;
    Ok(snapshot)
}

/// Creates a new, on-demand snapshot in the oplog.
///
/// - `project_id`: The ID of the project to create a snapshot for.
/// - `message`: Optional message to include with the snapshot.
///
/// Returns the OID of the created snapshot.
///
/// # Errors
/// Returns an error if the project cannot be found or if there is an issue creating the snapshot.
#[but_api]
#[instrument(err(Debug))]
pub fn create_snapshot(project_id: ProjectId, message: Option<String>) -> Result<gix::ObjectId> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut details = SnapshotDetails::new(OperationKind::OnDemandSnapshot);
    details.body = message;
    let oid = ctx.create_snapshot(details, guard.write_permission())?;
    Ok(oid.to_gix())
}

/// Restores the project to a specific snapshot. This operation also creates a new snapshot in the oplog.
///
/// - `project_id`: The ID of the project to restore.
/// - `sha`: The SHA of the snapshot to restore to.
///
/// # Errors
/// Returns an error if the project cannot be found, if the snapshot SHA is invalid, or if there is an issue during the restore operation.
///
/// # Side Effects
/// This operation modifies the repository state, reverting it to the specified snapshot.
/// This includes the state of the working directory as well as commmit history and references.
/// Additionally, a new snapshot is created in the oplog to record the restore action.
#[but_api]
#[instrument(err(Debug))]
pub fn restore_snapshot(project_id: ProjectId, sha: String) -> Result<()> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(
        sha.parse().map_err(anyhow::Error::from)?,
        guard.write_permission(),
    )?;
    Ok(())
}

/// Computes the file tree difference between the the state of the project at a specific snapshot and the current state.
/// Not all snapshots may have a meaningful file tree difference, in which case the result may be empty.
/// An example of a snapshot that does have file tree diffs is a `CreateCommit` snapshot where the commit introduced changes to files.
///
/// - `project_id`: The ID of the project to compute the diff for.
/// - `sha`: The SHA of the snapshot to diff against the current state.
///
/// Returns a vector of `TreeChange` entries representing the differences.
///
/// # Errors
/// Returns an error if the project cannot be found, if the snapshot SHA is invalid, or if there is an issue computing the diff.
#[but_api]
#[instrument(err(Debug))]
pub fn snapshot_diff(project_id: ProjectId, sha: String) -> Result<Vec<but_core::ui::TreeChange>> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let diff = ctx.snapshot_diff(sha.parse().map_err(anyhow::Error::from)?)?;
    let diff: Vec<but_core::ui::TreeChange> = diff.into_iter().map(Into::into).collect();
    Ok(diff)
}
