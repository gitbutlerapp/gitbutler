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
        atoms::{BranchArg, BranchOrCommit, CliIdArg, Purpose},
        move2::Platform,
    },
    bad_input,
    theme::{self, Theme},
    utils::{CliOutputHuman, IntermediateChannel, WriteWithUtils, targeting::Side},
};

pub struct MoveOutcome {
    pub sources: NonEmpty<gix::ObjectId>,
    pub target: Option<MoveTarget>,
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

        write!(out, "Moved {sources}")?;

        if let Some(new_branch_name) = new_branch_name {
            write!(out, " to new branch {}", theme::Branch(new_branch_name))?;
        }

        if let Some(target) = target {
            write!(out, " {target}")?;
        }

        writeln!(out)?;

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
    RelativeTo(MoveCommitsRelativeToOperation),
    ToNewBranch(MoveCommitsToNewBranchOperation),
}

#[derive(Clone)]
struct MoveCommitsRelativeToOperation {
    sources: NonEmpty<gix::ObjectId>,
    target: MoveTarget,
}

impl MoveCommitsRelativeToOperation {
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
            MoveTarget::BranchTip { name } => {
                (RelativeTo::Reference(name), Side::Below.into(), None)
            }
        };

        tx.move_commits(self.sources, relative_to, side)?;
        Ok(new_branch_name)
    }
}

#[derive(Clone)]
struct MoveCommitsToNewBranchOperation {
    sources: NonEmpty<gix::ObjectId>,
    branch_name: Option<FullName>,
}

impl MoveCommitsToNewBranchOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<FullName> {
        let new_branch_name = if let Some(branch_name) = self.branch_name {
            branch_name
        } else {
            but_core::branch::unique_canned_refname(tx.repo())?
        };
        tx.create_reference(
            new_branch_name.as_ref(),
            None,
            |_| StackId::generate(),
            Some(0),
        )?;
        tx.move_commits(
            self.sources,
            RelativeTo::Reference(new_branch_name.clone()),
            Side::Below.into(),
        )?;
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
    BranchTip {
        name: FullName,
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
            Self::BranchTip { name } => {
                write!(f, "to the tip of branch {}", theme::Branch(name))
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
        branch,
    } = args;

    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;

    match (branch, above, below) {
        (Some(Some(branch)), None, None) => {
            let resolved_sources = sources
                .into_iter()
                .map(|source| source.resolve_commit_in_workspace(&repo, id_map))
                .collect::<CliResult<Vec<_>>>()?;
            let sources = NonEmpty::from_vec(resolved_sources)
                .expect("BUG: Empty sources should not be possible as it's a required argument");

            match branch.try_resolve_branch(&repo, id_map)? {
                Some(branch) => Ok(MoveOperation::RelativeTo(MoveCommitsRelativeToOperation {
                    sources,
                    target: MoveTarget::BranchTip {
                        name: branch.resolve_local_branch_name()?,
                    },
                })),
                None => {
                    let branch_name =
                        BranchArg(branch.to_string()).resolve_for_creation(&repo, &ws)?;
                    Ok(MoveOperation::ToNewBranch(
                        MoveCommitsToNewBranchOperation {
                            sources,
                            branch_name: Some(branch_name),
                        },
                    ))
                }
            }
        }
        (Some(None), None, None) => {
            let resolved_sources = sources
                .into_iter()
                .map(|source| source.resolve_commit_in_workspace(&repo, id_map))
                .collect::<CliResult<Vec<_>>>()?;
            let sources = NonEmpty::from_vec(resolved_sources)
                .expect("BUG: Empty sources should not be possible as it's a required argument");
            Ok(MoveOperation::ToNewBranch(
                MoveCommitsToNewBranchOperation {
                    sources,
                    branch_name: None,
                },
            ))
        }
        (None, Some(above), None) => {
            create_move_above_or_below_op(&repo, id_map, sources, above, Side::Above)
        }
        (None, None, Some(below)) => {
            create_move_above_or_below_op(&repo, id_map, sources, below, Side::Below)
        }
        _ => unreachable!("BUG: Targeting group is required"),
    }
}

fn create_move_above_or_below_op(
    repo: &gix::Repository,
    id_map: &IdMap,
    sources: impl IntoIterator<Item = CliIdArg>,
    unresolved_target: CliIdArg,
    side: Side,
) -> CliResult<MoveOperation> {
    let target = {
        match unresolved_target
            .resolve_in_workspace(repo, id_map, Purpose::Anchor, None)?
            .into_branch_or_commit()?
        {
            BranchOrCommit::Commit(commit_id) => MoveTarget::Commit { commit_id, side },
            BranchOrCommit::Branch(branch_arg) => MoveTarget::BranchBucket {
                name: branch_arg.resolve_existing_local_branch(repo)?,
                side,
            },
        }
    };

    let sources = sources
        .into_iter()
        .map(|source| {
            let resolved_source = source.resolve_commit_in_workspace(repo, id_map)?;

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

    Ok(MoveOperation::RelativeTo(MoveCommitsRelativeToOperation {
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
                MoveOperation::RelativeTo(op) => op.execute(&mut tx)?,
                MoveOperation::ToNewBranch(op) => Some(op.execute(&mut tx)?),
            };

            Ok(but_transaction::Commit(new_branch_name))
        },
    )?;

    let outcome = match move_op {
        MoveOperation::RelativeTo(MoveCommitsRelativeToOperation { sources, target }) => {
            MoveOutcome {
                sources,
                target: Some(target),
                new_branch_name,
            }
        }
        MoveOperation::ToNewBranch(MoveCommitsToNewBranchOperation {
            sources,
            branch_name: _,
        }) => MoveOutcome {
            sources,
            target: None,
            new_branch_name,
        },
    };

    Ok(outcome)
}
