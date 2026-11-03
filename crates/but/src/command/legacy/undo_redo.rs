//! Implementation shared by the `but undo` and `but redo` commands.

use but_api::{json::HexHash, legacy::oplog::RestoreKind};
use but_core::sync::RepoExclusive;
use but_ctx::Context;
use gitbutler_oplog::{BranchInOtherWorktree, entry::OperationKind};
use serde::Serialize;

use crate::{
    CliResult,
    args::{redo, undo},
    bad_input,
    error::{BadInput, REVERSE_OPERATION_HINT},
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, change_source::InvokedFrom,
        shorten_object_id,
    },
};

#[derive(Debug, Copy, Clone)]
pub enum Direction {
    Undo,
    Redo,
}

impl Direction {
    fn action(self) -> &'static str {
        match self {
            Direction::Undo => "undo",
            Direction::Redo => "redo",
        }
    }

    fn past_tense(self) -> &'static str {
        match self {
            Direction::Undo => "Undid",
            Direction::Redo => "Redid",
        }
    }

    fn nothing_to_restore_message(self) -> &'static str {
        match self {
            Direction::Undo => "No previous operations to undo.",
            Direction::Redo => "No previous undo to redo.",
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Operation {
    Undo,
    Redo,
}

#[must_use]
pub enum UndoRedoOutcome {
    Restored {
        direction: Direction,
        snapshot_id: gix::ObjectId,
        target_operation: OperationKind,
        target_time: gix::date::Time,
    },
    NothingToRestore {
        direction: Direction,
    },
}

impl CliOutputHuman for UndoRedoOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            UndoRedoOutcome::Restored {
                direction,
                snapshot_id,
                target_operation,
                target_time,
            } => {
                writeln!(
                    out,
                    "{} {} ({}): {}",
                    direction.past_tense(),
                    theme::Commit(snapshot_id),
                    snapshot_time_string(target_time),
                    target_operation.title(),
                )?;
            }
            UndoRedoOutcome::NothingToRestore { direction } => {
                writeln!(out, "{}", direction.nothing_to_restore_message())?;
            }
        }

        Ok(())
    }
}

impl CliOutput for UndoRedoOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            action: &'static str,
            changed: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            snapshot_id: Option<HexHash>,
        }

        match self {
            UndoRedoOutcome::Restored {
                direction,
                snapshot_id,
                target_operation: _,
                target_time: _,
            } => Output {
                action: direction.action(),
                changed: true,
                snapshot_id: Some(snapshot_id.into()),
            },
            UndoRedoOutcome::NothingToRestore { direction } => Output {
                action: direction.action(),
                changed: false,
                snapshot_id: None,
            },
        }
    }
}

pub fn undo(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: undo::Platform,
    invoked_from: &InvokedFrom,
) -> CliResult<UndoRedoOutcome> {
    run_from(ctx, resolve_undo(args)?, invoked_from)
}

pub fn redo(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: redo::Platform,
    invoked_from: &InvokedFrom,
) -> CliResult<UndoRedoOutcome> {
    run_from(ctx, resolve_redo(args)?, invoked_from)
}

/// [`run`], with a refusal that says so when the branch in the way is the one this linked worktree
/// has checked out, as the repository `run` restores is always the main checkout.
fn run_from(
    ctx: &mut Context,
    operation: Operation,
    invoked_from: &InvokedFrom,
) -> CliResult<UndoRedoOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let result = run(ctx, guard.write_permission(), operation);
    if let Err(error) = &result
        && let Some(refusal) = error.downcast_ref::<BranchInOtherWorktree>()
        && refusal
            .dirs
            .iter()
            .all(|dir| invoked_from.is_rooted_at(dir))
    {
        return Err(refused_in_this_worktree(ctx, operation, refusal)?.into());
    }
    Ok(result?)
}

fn refused_in_this_worktree(
    ctx: &Context,
    operation: Operation,
    refusal: &BranchInOtherWorktree,
) -> anyhow::Result<BadInput> {
    let branch = refusal.branch.shorten();
    let (command, commit) = match operation {
        Operation::Undo => ("but undo", commit_to_uncommit(ctx, refusal)?),
        Operation::Redo => ("but redo", None),
    };
    let hint = match commit {
        Some(commit) => format!(
            "Run `but uncommit {commit}` to take the commit off '{branch}' and keep its changes uncommitted"
        ),
        None => REVERSE_OPERATION_HINT.to_owned(),
    };
    Ok(bad_input(format!(
        "Cannot restore branch '{branch}' because it is checked out in this linked worktree, \
        and `{command}` only restores the main checkout"
    ))
    .hint(hint))
}

/// The commit `refusal` is about if `but undo` wants to reverse creating it: the last operation
/// made a commit, and that commit is all that `refusal`'s branch gained since. A commit without
/// GitButler's headers came from elsewhere, so that operation didn't make it.
fn commit_to_uncommit(
    ctx: &Context,
    refusal: &BranchInOtherWorktree,
) -> anyhow::Result<Option<String>> {
    let made_a_commit = but_api::legacy::oplog::get_undo_target_snapshot(ctx)?
        .and_then(|snapshot| snapshot.details)
        .is_some_and(|details| details.operation == OperationKind::CreateCommit);
    if !made_a_commit {
        return Ok(None);
    }
    let repo = ctx.repo.get()?;
    let tip = but_core::Commit::try_from(
        repo.find_reference(refusal.branch.as_ref())?
            .peel_to_commit()?,
    )?;
    let gained_only_tip = refusal
        .restored_tip
        .is_none_or(|restored| tip.parents.first() == Some(&restored));
    Ok((tip.headers().is_some() && gained_only_tip)
        .then(|| shorten_object_id(&repo, tip.id.detach())))
}

fn resolve_undo(undo::Platform {}: undo::Platform) -> CliResult<Operation> {
    Ok(Operation::Undo)
}

fn resolve_redo(redo::Platform {}: redo::Platform) -> CliResult<Operation> {
    Ok(Operation::Redo)
}

pub fn run(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    operation: Operation,
) -> anyhow::Result<UndoRedoOutcome> {
    let (direction, target_snapshot, restore_kind) = match operation {
        Operation::Undo => (
            Direction::Undo,
            but_api::legacy::oplog::get_undo_target_snapshot(ctx)?,
            RestoreKind::RestoreFromSnapshotViaUndo,
        ),
        Operation::Redo => (
            Direction::Redo,
            but_api::legacy::oplog::get_redo_target_snapshot(ctx)?,
            RestoreKind::RestoreFromSnapshotViaRedo,
        ),
    };

    let Some(target_snapshot) = target_snapshot else {
        return Ok(UndoRedoOutcome::NothingToRestore { direction });
    };

    let restore_snapshot_id = target_snapshot.commit_id;
    let operation_snapshot =
        but_api::legacy::oplog::peel_restore_snapshot(ctx, restore_snapshot_id)?
            .unwrap_or(target_snapshot);
    let snapshot_id = operation_snapshot.commit_id;
    let target_operation = operation_snapshot
        .details
        .as_ref()
        .map(|details| details.operation)
        .unwrap_or(OperationKind::Unknown);
    let target_time = operation_snapshot.created_at;

    but_api::legacy::oplog::restore_snapshot_with_kind_with_perm(
        ctx,
        restore_kind,
        restore_snapshot_id,
        perm,
    )?;

    Ok(UndoRedoOutcome::Restored {
        direction,
        snapshot_id,
        target_operation,
        target_time,
    })
}

fn snapshot_time_string(time: gix::date::Time) -> String {
    time.format(super::oplog::ISO8601_NO_TZ)
        .unwrap_or_else(|_| time.seconds.to_string())
}
