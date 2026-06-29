use std::fmt::Display;

use but_core::{DryRun, RefMetadata, ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_transaction::Transaction;
use but_workspace::branch::create_reference::Anchor;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::refs::FullName;
use itertools::Itertools;
use nonempty::NonEmpty;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{BranchOrCommit, Purpose},
        move2::Platform,
    },
    bad_input,
    theme::{self, Theme},
    utils::{CliOutputHuman, IntermediateChannel, WriteWithUtils, targeting::Side},
};

pub struct MoveOutcome {
    pub sources: NonEmpty<gix::ObjectId>,
    pub target: MoveTarget,
    pub new_branch_name: Option<FullName>,
}

impl CliOutputHuman for MoveOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _theme: &Theme) -> anyhow::Result<()> {
        let Self {
            sources,
            target,
            new_branch_name,
        } = self;
        let sources = sources.into_iter().map(theme::Commit).join(", ");

        if let Some(new_branch_name) = new_branch_name {
            writeln!(
                out,
                "Moved {sources} to new branch {} {target}",
                theme::Branch(new_branch_name)
            )?;
        } else {
            writeln!(out, "Moved {sources} {target}")?;
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
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<Option<FullName>> {
        let (relative_to, side, new_branch_name) = match self.target {
            MoveTarget::Commit { commit_id, side } => {
                (RelativeTo::Commit(commit_id), side.into(), None)
            }
            MoveTarget::BranchBucket { name, side } => {
                let new_branch_name = but_core::branch::unique_canned_refname(tx.repo())?;
                let anchor = Anchor::at_segment(name.as_ref(), side.into());
                tx.create_reference(
                    new_branch_name.as_ref(),
                    anchor,
                    |_| StackId::generate(),
                    Some(0),
                )?;
                (
                    RelativeTo::Reference(new_branch_name.clone()),
                    Side::Below.into(),
                    Some(new_branch_name),
                )
            }
        };

        tx.move_commits(self.sources, relative_to, side)?;
        Ok(new_branch_name)
    }
}

#[derive(Clone)]
pub enum MoveTarget {
    /// Place the commit relative to this commit, within the same branch.
    Commit {
        commit_id: gix::ObjectId,
        side: Side,
    },
    BranchBucket {
        name: FullName,
        side: Side,
    },
}

impl Display for MoveTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commit { commit_id, side } => {
                write!(f, "{} commit {}", side, theme::Commit(*commit_id))
            }
            Self::BranchBucket { name, side } => {
                write!(f, "{} branch {}", side, theme::Branch(name))
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
        match unresolved_target
            .resolve_in_workspace(&repo, id_map, Purpose::Anchor, None)?
            .into_branch_or_commit()?
        {
            BranchOrCommit::Commit(commit_id) => MoveTarget::Commit { commit_id, side },
            BranchOrCommit::Branch(branch_arg) => MoveTarget::BranchBucket {
                name: branch_arg.resolve_existing_local_branch(&repo)?,
                side,
            },
        }
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
    let (new_branch_name, _ws) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let new_branch_name = match move_op.clone() {
                MoveOperation::Commit(op) => op.execute(&mut tx)?,
            };

            Ok(but_transaction::Commit(new_branch_name))
        },
    )?;

    let outcome = match move_op {
        MoveOperation::Commit(MoveCommitsOperation { sources, target }) => MoveOutcome {
            sources,
            target,
            new_branch_name,
        },
    };

    Ok(outcome)
}
