//! The operation log: what GitButler did to a project, newest first, each with the
//! snapshot taken just before, to look at and to restore.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use but_api::legacy::oplog::{self, RestoreKind};
use but_oplog::legacy::OperationKind;

use super::{SnapshotId, open, short_ref};

/// One operation in the log.
#[derive(Debug, Clone)]
pub struct Operation {
    /// The snapshot taken just before it.
    pub id: SnapshotId,
    /// What was done, like "Created commit".
    pub title: String,
    /// More about it: a snapshot's message, or the operation an undo took back.
    pub detail: Option<String>,
    /// When, in seconds since the epoch.
    pub time: i64,
    /// What kind of operation, when the oplog says.
    pub kind: Option<OperationKind>,
}

/// Up to `limit` operations older than `after`, or the newest ones; and whether more remain.
pub fn list(
    path: &Path,
    after: Option<SnapshotId>,
    limit: usize,
) -> Result<(Vec<Operation>, bool)> {
    let ctx = open(path)?;
    let mut snapshots = oplog::list_snapshots(&ctx, limit + 1, after.map(|s| s.0), None, None)?;
    let more = snapshots.len() > limit;
    snapshots.truncate(limit);
    let operations = snapshots
        .into_iter()
        .map(|snapshot| {
            // An undo, redo or restore says which operation it took back.
            let restored = |title: &str| {
                let restored = oplog::peel_restore_snapshot(&ctx, snapshot.commit_id)
                    .ok()
                    .flatten()
                    .and_then(|peeled| peeled.details)
                    .map(|d| operation_title(d.operation).to_owned());
                (title.to_owned(), restored)
            };
            use OperationKind as K;
            let kind = snapshot.details.as_ref().map(|details| details.operation);
            let (title, detail) = match snapshot.details {
                Some(details) => match details.operation {
                    K::RestoreFromSnapshotViaUndo => restored("Undo"),
                    K::RestoreFromSnapshotViaRedo => restored("Redo"),
                    K::RestoreFromSnapshot => restored("Restore"),
                    _ => {
                        // What it was done to: a commit's title, a branch, a file.
                        let about = ["message", "name", "branch", "file"]
                            .iter()
                            .find_map(|key| {
                                let trailer = details.trailers.iter().find(|t| t.key() == *key)?;
                                let value = trailer.value();
                                let line = value.lines().next()?;
                                Some(short_ref(line).to_owned())
                            });
                        let body = details.body.filter(|b| !b.trim().is_empty());
                        (
                            operation_title(details.operation).to_owned(),
                            about.or(body),
                        )
                    }
                },
                None => ("Unknown operation".to_owned(), None),
            };
            Operation {
                id: SnapshotId(snapshot.commit_id),
                title,
                detail,
                time: snapshot.created_at.seconds,
                kind,
            }
        })
        .collect();
    Ok((operations, more))
}

/// Why undo and restore refuse to take back one of the operations kept from them.
const UNRECORDED: &str = "GitButler's history records only the workspace and the main \
    checkout, not linked worktrees or branches outside it. Change it back by hand instead.";

/// Put the project back to how it was before the operation `id` was taken for, refusing
/// when that takes back one of `kept`, as undo does. The restore is itself recorded, so it
/// can be undone.
pub fn restore(path: &Path, id: SnapshotId, kept: &HashSet<SnapshotId>) -> Result<()> {
    let mut ctx = open(path)?;
    // Each snapshot is made on the one before, so one taken since `id` has it as its base.
    let repo = ctx.repo.get()?;
    let since = |snapshot: &SnapshotId| {
        snapshot.0 == id.0
            || repo
                .merge_base(id.0, snapshot.0)
                .is_ok_and(|base| base.is_some_and(|b| b == id.0))
    };
    if kept.iter().any(since) {
        bail!(UNRECORDED);
    }
    drop(repo);
    oplog::restore_snapshot_with_kind(&mut ctx, RestoreKind::ExplicitRestoreFromSnapshot, id.0)
}

/// Record the project as it is now.
pub fn take_snapshot(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    oplog::create_snapshot(&mut ctx, None)?;
    Ok(())
}

/// Restore the workspace to before the last operation, or with `redo` restore what the
/// last undo took back, refusing when that operation is one of `kept`. Returns what it was,
/// or `None` when there's nothing to undo or redo.
pub fn undo(path: &Path, redo: bool, kept: &HashSet<SnapshotId>) -> Result<Option<&'static str>> {
    let mut ctx = open(path)?;
    let (snapshot, kind) = if redo {
        let snapshot = oplog::get_redo_target_snapshot(&ctx)?;
        (snapshot, RestoreKind::RestoreFromSnapshotViaRedo)
    } else {
        let snapshot = oplog::get_undo_target_snapshot(&ctx)?;
        (snapshot, RestoreKind::RestoreFromSnapshotViaUndo)
    };
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    if kept.contains(&SnapshotId(snapshot.commit_id)) {
        bail!(UNRECORDED);
    }
    let title = snapshot
        .details
        .map(|details| operation_title(details.operation));
    restore_titled(&mut ctx, snapshot.commit_id, title, kind).map(Some)
}

/// The newest entry in the project's operation log.
pub fn oplog_head(path: &Path) -> Option<SnapshotId> {
    let ctx = open(path).ok()?;
    let newest = oplog::list_snapshots(&ctx, 1, None, None, None).ok()?;
    newest
        .first()
        .map(|snapshot| SnapshotId(snapshot.commit_id))
}

/// Undo the operation that recorded `snapshot`, but only while it's the latest thing to
/// undo: the log is shared with the CLI and agents, and restoring anything else would take
/// back more than the user asked for.
pub fn undo_snapshot(path: &Path, snapshot: SnapshotId) -> Result<&'static str> {
    let mut ctx = open(path)?;
    let target = oplog::get_undo_target_snapshot(&ctx)?
        .filter(|target| target.commit_id == snapshot.0)
        .context("Something else changed the workspace since. ⌘Z undoes one step at a time.")?;
    let title = target
        .details
        .map(|details| operation_title(details.operation));
    let kind = RestoreKind::RestoreFromSnapshotViaUndo;
    restore_titled(&mut ctx, snapshot.0, title, kind)
}

/// What the app calls an operation, where the oplog's name for it is another client's.
fn operation_title(kind: OperationKind) -> &'static str {
    use OperationKind as K;
    match kind {
        K::UndoCommit => "Uncommitted",
        K::InsertBlankCommit => "Empty commit added",
        K::MergeUpstream | K::UpdateWorkspaceBase => "Updated from target",
        K::SetBaseBranch => "Changed target",
        other => other.title(),
    }
}

/// Restore snapshot `id` and name the operation it takes back, as the CLI does: when `id`
/// is itself an undo or redo, it's the operation that one restored.
fn restore_titled(
    ctx: &mut but_ctx::Context,
    id: gix::ObjectId,
    title: Option<&'static str>,
    kind: oplog::RestoreKind,
) -> Result<&'static str> {
    let title = oplog::peel_restore_snapshot(ctx, id)?
        .map_or(title, |peeled| {
            peeled
                .details
                .map(|details| operation_title(details.operation))
        })
        .unwrap_or("Unknown operation");
    oplog::restore_snapshot_with_kind(ctx, kind, id)?;
    Ok(title)
}
