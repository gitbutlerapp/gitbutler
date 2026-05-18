//! GitButler has an operation log (oplog) that records significant actions taken within a project.
//! The oplog records snapshots of the project state at the time when an operation was initiated.
//!
//! Using the oplog, it is possible to restore the project to a previous state by reverting to one of these snapshots.
//! This includes the state of the working directory as well as commit history and references.
//!
//! This module provides commands to interact with the oplog, including listing snapshots, creating new snapshots,
//! restoring to a snapshot, and viewing differences between snapshots.
//!
//! An example usage:
//!   - A user squashes two commits together.
//!   - Listing the snapshot will show a new snapshot with the operation kind `SquashCommit`.
//!   - Restoring to the snapshot will revert the project to the state before the squash.
//!   - A new snapshot is created for the restore operation.
//!
//! Depending on the snapshot operation kind, there may be a payload (body) with additional details about the operation (e.g. commit message).
//! Refer to `gitbutler_oplog::entry::Snapshot` and `gitbutler_oplog::entry::SnapshotDetails` for the metadata stored.
//!
use anyhow::Result;
use but_api_macros::but_api;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, Snapshot, SnapshotDetails},
};
use tracing::instrument;

mod json {
    use but_oplog::legacy::OperationKind;
    use serde::Serialize;

    use crate::json::HexHash;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    pub(super) struct Snapshot {
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        pub commit_id: HexHash,
        pub created_at: i128,
        pub details: Option<SnapshotDetails>,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(Snapshot);

    #[derive(Debug, PartialEq, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    pub(super) struct SnapshotDetails {
        pub(super) version: u32,
        pub(super) operation: OperationKind,
        pub(super) title: String,
        pub(super) body: Option<String>,
        pub(super) trailers: Vec<Trailer>,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(SnapshotDetails);

    #[derive(Debug, PartialEq, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    pub(super) struct Trailer {
        pub(super) key: String,
        pub(super) value: String,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(Trailer);

    impl TryFrom<gitbutler_oplog::entry::Snapshot> for Snapshot {
        type Error = anyhow::Error;

        fn try_from(value: gitbutler_oplog::entry::Snapshot) -> Result<Self, Self::Error> {
            let gitbutler_oplog::entry::Snapshot {
                commit_id,
                created_at,
                details,
            } = value;

            Ok(Self {
                commit_id: commit_id.into(),
                created_at: i128::from(created_at.seconds) * 1000,
                details: details.map(Into::into),
            })
        }
    }

    impl From<gitbutler_oplog::entry::SnapshotDetails> for SnapshotDetails {
        fn from(value: gitbutler_oplog::entry::SnapshotDetails) -> Self {
            let gitbutler_oplog::entry::SnapshotDetails {
                version,
                operation,
                title,
                body,
                trailers,
            } = value;

            Self {
                version: version.0,
                operation,
                title,
                body,
                trailers: trailers.into_iter().map(Into::into).collect(),
            }
        }
    }

    impl From<gitbutler_oplog::entry::Trailer> for Trailer {
        fn from(value: gitbutler_oplog::entry::Trailer) -> Self {
            Trailer {
                key: value.key().to_owned(),
                value: value.value().to_string(),
            }
        }
    }
}

/// List snapshots in the oplog.
///
/// - `limit`: Maximum number of snapshots to return.
/// - `sha`: Optional SHA to filter snapshots starting after a specific commit.
/// - `exclude_kind`: Optional list of operation kinds to exclude from the results.
/// - `include_kind`: Optional list of operation kinds to include (if set, only these kinds are returned).
///
/// Returns a vector of `Snapshot` entries.
///
/// Prefer using [`snapshots_iter`] if possible.
///
/// # Errors
/// Returns an error if the project cannot be found or if there is an issue accessing the oplog.
#[but_api]
#[instrument(err(Debug))]
pub fn list_snapshots(
    ctx: &but_ctx::Context,
    limit: usize,
    sha: Option<gix::ObjectId>,
    exclude_kind: Option<Vec<OperationKind>>,
    include_kind: Option<Vec<OperationKind>>,
) -> Result<Vec<Snapshot>> {
    let snapshots = ctx
        .snapshots_iter(sha, exclude_kind.unwrap_or_default(), include_kind)?
        .take(limit)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(snapshots)
}

/// Iterate snapshots in the oplog.
///
/// - `sha`: Optional SHA to filter snapshots starting after a specific commit.
/// - `exclude_kind`: Optional list of operation kinds to exclude from the results.
/// - `include_kind`: Optional list of operation kinds to include (if set, only these kinds are returned).
///
/// Returns an iterator of `Snapshot` entries.
///
/// # Errors
/// Returns an error if the project cannot be found or if there is an issue accessing the oplog.
#[instrument(err(Debug))]
pub fn snapshots_iter(
    ctx: &but_ctx::Context,
    sha: Option<gix::ObjectId>,
    exclude_kind: Option<Vec<OperationKind>>,
    include_kind: Option<Vec<OperationKind>>,
) -> Result<impl Iterator<Item = Result<Snapshot>>> {
    ctx.snapshots_iter(sha, exclude_kind.unwrap_or_default(), include_kind)
}

/// Get the snapshot that an undo operation should restore to.
///
/// This handles multiple consecutive undos.
#[but_api(napi, try_from = json::Snapshot)]
#[instrument(err(Debug))]
pub fn get_undo_target_snapshot(ctx: &but_ctx::Context) -> Result<Option<Snapshot>> {
    // Undo snapshots are bookkeeping entries, not operations we should restore to directly. Walk
    // newest-to-oldest and let each undo entry cancel one older real operation.
    //
    // This handles both repeated undos:
    //
    //     [UNDO]   <- skip one real operation
    //     [UNDO]   <- skip one real operation
    //     [REWORD] <- skipped
    //     [REWORD] <- skipped
    //     [REWORD] <- target
    //
    // And a new operation after an undo:
    //
    //     [UNDO]   <- skip one real operation
    //     [REWORD] <- skipped
    //     [UNDO]   <- skip one more real operation, not a valid target itself
    //     [REWORD] <- skipped
    //     [REWORD] <- target
    let snapshots = ctx.snapshots_iter(None, Vec::new(), None)?;
    let mut real_operations_to_skip = 0_usize;

    for snapshot in snapshots {
        let snapshot = snapshot?;

        if snapshot.details.as_ref().is_some_and(|details| {
            matches!(details.operation, OperationKind::RestoreFromSnapshotViaUndo)
        }) {
            real_operations_to_skip += 1;
            continue;
        }

        if real_operations_to_skip == 0 {
            return Ok(Some(snapshot));
        }
        real_operations_to_skip -= 1;
    }

    Ok(None)
}

/// Get the snapshot that a redo operation should restore to.
///
/// This handles multiple consecutive redos.
#[but_api(napi, try_from = json::Snapshot)]
#[instrument(err(Debug))]
pub fn get_redo_target_snapshot(ctx: &but_ctx::Context) -> Result<Option<Snapshot>> {
    // Redo snapshots are bookkeeping entries, not operations we should restore to directly. Walk
    // newest-to-oldest and let each redo entry cancel one older undo entry.
    //
    // This handles repeated redos:
    //
    //     [REDO] <- skip one undo
    //     [REDO] <- skip one undo
    //     [UNDO] <- skipped
    //     [UNDO] <- skipped
    //     [UNDO] <- target
    //
    // And stops redo when a new real operation happened after an undo:
    //
    //     [REWORD] <- not an undo, so there is no redo target
    //     [UNDO]
    //     [REWORD]
    let snapshots = ctx.snapshots_iter(None, Vec::new(), None)?;
    let mut undos_to_skip = 0_usize;

    for snapshot in snapshots {
        let snapshot = snapshot?;

        if snapshot.details.as_ref().is_some_and(|details| {
            matches!(details.operation, OperationKind::RestoreFromSnapshotViaRedo)
        }) {
            undos_to_skip += 1;
            continue;
        }

        if undos_to_skip == 0 {
            if snapshot.details.as_ref().is_some_and(|details| {
                !matches!(details.operation, OperationKind::RestoreFromSnapshotViaUndo)
            }) {
                return Ok(None);
            }
            return Ok(Some(snapshot));
        }
        undos_to_skip -= 1;
    }

    Ok(None)
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
pub fn get_snapshot(ctx: &but_ctx::Context, sha: gix::ObjectId) -> Result<Snapshot> {
    let snapshot = ctx.get_snapshot(sha)?;
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
pub fn create_snapshot(
    ctx: &mut but_ctx::Context,
    message: Option<String>,
) -> Result<gix::ObjectId> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut details = SnapshotDetails::new(OperationKind::OnDemandSnapshot);
    details.body = message;
    let oid = ctx.create_snapshot(details, guard.write_permission())?;
    Ok(oid)
}

pub use gitbutler_oplog::RestoreKind;

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
/// This includes the state of the working directory as well as commit history and references.
/// Additionally, a new snapshot is created in the oplog to record the restore action.
#[but_api]
#[instrument(err(Debug))]
pub fn restore_snapshot(ctx: &mut but_ctx::Context, sha: gix::ObjectId) -> Result<()> {
    restore_snapshot_with_kind(ctx, RestoreKind::ExplicitRestoreFromSnapshot, sha)
}

/// Restores the project to a specific snapshot using a specific kind of restore. This operation
/// also creates a new snapshot in the oplog.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn restore_snapshot_with_kind(
    ctx: &mut but_ctx::Context,
    restore_kind: RestoreKind,
    sha: gix::ObjectId,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(sha, restore_kind, guard.write_permission())?;
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
pub fn snapshot_diff(
    ctx: &but_ctx::Context,
    sha: gix::ObjectId,
    child_id: Option<gix::ObjectId>,
) -> Result<Vec<but_core::ui::TreeChange>> {
    let diff = ctx.snapshot_diff(sha, child_id)?;
    let diff: Vec<but_core::ui::TreeChange> = diff.into_iter().map(Into::into).collect();
    Ok(diff)
}

/// Find the final snapshot that a restore snapshot will restore from.
///
/// For example if you do a reword and then a series of undos and redos the oplog would look like this:
///
/// 9ea77ad REDO
/// 71c6be6 UNDO
/// c33acf3 REDO
/// 3a0c4d1 UNDO
/// bd1724b REWORD
///
/// and `peel_restore_snapshot` will return the snapshot for `bd1724b`.
///
/// If the given snapshot is not a restore snapshot then the same snapshot will be returned.
#[but_api(napi, try_from = json::Snapshot)]
#[instrument(err(Debug))]
pub fn peel_restore_snapshot(
    ctx: &but_ctx::Context,
    sha: gix::ObjectId,
) -> Result<Option<Snapshot>> {
    let snapshot = get_snapshot(ctx, sha)?;
    gitbutler_oplog::peel_restore_snapshot(ctx, &snapshot)
}
