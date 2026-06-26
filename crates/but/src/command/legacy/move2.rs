use std::fmt::Display;

use but_core::{DryRun, RefMetadata, sync::RepoExclusive};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_transaction::Transaction;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use itertools::Itertools;
use nonempty::NonEmpty;

use crate::{
    CliResult, IdMap,
    args::move2::Platform,
    bad_input,
    theme::{self, Theme},
    utils::{CliOutputHuman, IntermediateChannel, WriteWithUtils, targeting::Side},
};

pub enum MoveOutcome {
    Commits {
        sources: NonEmpty<gix::ObjectId>,
        target: MoveTarget,
    },
}

impl CliOutputHuman for MoveOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _theme: &Theme) -> anyhow::Result<()> {
        match self {
            Self::Commits { sources, target } => {
                let sources = sources.into_iter().map(theme::Commit).join(", ");

                writeln!(out, "Moved {sources} {target}")?;
            }
        }

        Ok(())
    }
}

pub fn r#move(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<MoveOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let move_op = resolve(ctx, guard.write_permission(), args, &id_map)?;

    run(ctx, &mut meta, guard.write_permission(), move_op)
}

#[derive(Clone)]
enum MoveOperation {
    Commit(MoveCommitsOperation),
}

#[derive(Clone)]
struct MoveCommitsOperation {
    sources: NonEmpty<gix::ObjectId>,
    target: MoveTarget,
}

impl MoveCommitsOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<()> {
        let (relative_to, side) = match self.target {
            MoveTarget::Commit { commit_id, side } => (RelativeTo::Commit(commit_id), side.into()),
        };

        tx.move_commits(self.sources, relative_to, side)?;
        Ok(())
    }
}

#[derive(Clone)]
pub enum MoveTarget {
    /// Place the commit relative to this commit, within the same branch.
    Commit {
        commit_id: gix::ObjectId,
        side: Side,
    },
}

impl Display for MoveTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commit { commit_id, side } => {
                write!(f, "{} {}", side, theme::Commit(*commit_id))
            }
        }
    }
}

fn resolve(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    args: Platform,
    id_map: &IdMap,
) -> CliResult<MoveOperation> {
    let Platform {
        above,
        below,
        sources,
    } = args;

    let (repo, _ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;

    let (unresolved_target, side) = match (above, below) {
        (Some(above), None) => (above, Side::Above),
        (None, Some(below)) => (below, Side::Below),
        _ => unimplemented!(),
    };

    let target = {
        let commit_id = unresolved_target.resolve_commit_in_workspace(&repo, id_map)?;
        MoveTarget::Commit { commit_id, side }
    };

    let sources = sources
        .into_iter()
        .map(|source| {
            let resolved_source = source.resolve_commit_in_workspace(&repo, id_map)?;

            match &target {
                MoveTarget::Commit { commit_id, .. } if commit_id == &resolved_source => {
                    return Err(bad_input("Source cannot also be target")
                        .arg_value(source.to_string())
                        .arg_name(format!("--{side}"))
                        .hint(format!("Trying to move items {side} '{source}'? Remove '{source}' from '<SOURCES>' and try again!"))
                        .into());
                }
                _ => (),
            }

            Ok(resolved_source)
        })
        .collect::<CliResult<Vec<_>>>()?;
    let sources = NonEmpty::from_vec(sources)
        .expect("BUG: Empty sources should not be possible as it's a required argument");

    Ok(MoveOperation::Commit(MoveCommitsOperation {
        sources,
        target,
    }))
}

fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    move_op: MoveOperation,
) -> CliResult<MoveOutcome> {
    let snapshot_details = SnapshotDetails::new(OperationKind::MoveCommit);
    let ((), _ws) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            match move_op.clone() {
                MoveOperation::Commit(op) => op.execute(&mut tx)?,
            };

            Ok(but_transaction::Commit(()))
        },
    )?;

    let outcome = match move_op {
        MoveOperation::Commit(MoveCommitsOperation { sources, target }) => {
            MoveOutcome::Commits { sources, target }
        }
    };

    Ok(outcome)
}
