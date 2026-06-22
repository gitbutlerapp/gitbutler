use but_api::json::HexHash;
use but_core::{DryRun, RefMetadata, sync::RepoExclusive};
use but_ctx::Context;
use but_transaction::{DynamicOutcome, Transaction};
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::refs::FullName;
use itertools::Itertools;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::squash2::Platform,
    command::legacy::reword2::RewordCommitOperation,
    theme::{self, Theme},
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

pub enum SquashOutcome {
    Commits {
        sources: Vec<gix::ObjectId>,
        target: gix::ObjectId,
        new_commit: gix::ObjectId,
    },
    Branch {
        new_commit: gix::ObjectId,
        branch_name: FullName,
    },
}

impl CliOutputHuman for SquashOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _theme: &Theme) -> anyhow::Result<()> {
        match self {
            SquashOutcome::Commits {
                sources,
                target,
                new_commit,
            } => {
                let sources = sources.into_iter().map(theme::Commit).join(", ");

                writeln!(
                    out,
                    "Squashed {} into {} to create {}",
                    sources,
                    theme::Commit(target),
                    theme::Commit(new_commit)
                )?;
            }
            SquashOutcome::Branch {
                new_commit,
                branch_name,
            } => {
                writeln!(
                    out,
                    "Squashed branch {} to create commit {}",
                    theme::Branch(branch_name),
                    theme::Commit(new_commit)
                )?;
            }
        };

        Ok(())
    }
}

impl CliOutput for SquashOutcome {
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()> {
        let new_commit = match self {
            SquashOutcome::Commits { new_commit, .. }
            | SquashOutcome::Branch { new_commit, .. } => new_commit,
        };

        writeln!(out, "{new_commit}")?;

        Ok(())
    }

    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        struct Output {
            new_commit: HexHash,
        }

        let new_commit = match self {
            SquashOutcome::Commits { new_commit, .. }
            | SquashOutcome::Branch { new_commit, .. } => new_commit,
        };

        Output {
            new_commit: HexHash(new_commit),
        }
    }
}

pub fn squash(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<SquashOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let (squash_op, reword_op) = resolve(ctx, guard.write_permission(), args, &id_map)?;
    let outcome = run(
        ctx,
        &mut meta,
        guard.write_permission(),
        squash_op,
        reword_op,
    )?;

    Ok(outcome)
}

fn resolve(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    args: Platform,
    id_map: &IdMap,
) -> CliResult<(SquashOperation, Option<RewordCommitOperation>)> {
    let Platform {
        target,
        sources,
        message,
        no_message,
        use_target_message,
        use_source_message,
    } = args;

    let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm.read_permission())?;

    let (target, sources, branch_name) = if let Some(target) = target {
        let target = target.resolve_commit_in_workspace(&repo, id_map)?;
        let sources = sources
            .into_iter()
            .map(|source| source.resolve_commit_in_workspace(&repo, id_map))
            .collect::<CliResult<Vec<_>>>()?;
        (target, sources, None)
    } else {
        // TODO more error handling
        let source_branch = sources[0].resolve_branch_in_workspace(&repo, id_map)?;
        let source_branch_name = source_branch.resolve_local_branch_name()?;

        let (_, segment) = ws.try_find_segment_and_stack_by_refname(source_branch_name.as_ref())?;
        let mut sources = segment.commits.clone();
        let target = sources.pop().expect("HEHE :)");

        (
            target.id,
            sources.into_iter().map(|s| s.id).collect(),
            Some(source_branch_name),
        )
    };

    let (how_to_combine_messages, reword_op) = if use_target_message {
        (Some(MessageCombinationStrategy::KeepTarget), None)
    } else if use_source_message {
        (Some(MessageCombinationStrategy::KeepSubject), None)
    } else {
        (
            None,
            Some(RewordCommitOperation::resolve(no_message, message)),
        )
    };

    let squash_op = if let Some(branch_name) = branch_name {
        SquashOperation::SquashBranch(SquashBranchOperation {
            sources,
            target,
            how_to_combine_messages,
            branch_name,
        })
    } else {
        SquashOperation::SquashCommits(SquashCommitsOperation {
            sources,
            target,
            how_to_combine_messages,
        })
    };

    Ok((squash_op, reword_op))
}

fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    squash_op: SquashOperation,
    reword_op: Option<RewordCommitOperation>,
) -> CliResult<SquashOutcome> {
    let (sources, target, maybe_branch_name) = match squash_op.clone() {
        SquashOperation::SquashCommits(SquashCommitsOperation {
            sources,
            target,
            how_to_combine_messages: _,
        }) => (sources, target, None),
        SquashOperation::SquashBranch(SquashBranchOperation {
            sources,
            target,
            how_to_combine_messages: _,
            branch_name,
        }) => (sources, target, Some(branch_name)),
    };

    let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);
    let result = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let new_commit = match squash_op {
                SquashOperation::SquashCommits(squash_commits_operation) => {
                    squash_commits_operation.execute(&mut tx)?
                }
                SquashOperation::SquashBranch(squash_branch_operation) => {
                    squash_branch_operation.execute(&mut tx)?
                }
            };

            let new_commit = if let Some(reword_op) = reword_op {
                reword_op.execute(new_commit, &mut tx)?
            } else {
                new_commit
            };

            Ok(DynamicOutcome::<_, std::convert::Infallible>::Commit(
                new_commit,
            ))
        },
    )?;

    let DynamicOutcome::Commit((new_commit, _ws)) = result;

    match maybe_branch_name {
        Some(branch_name) => Ok(SquashOutcome::Branch {
            new_commit,
            branch_name,
        }),
        None => Ok(SquashOutcome::Commits {
            new_commit,
            sources,
            target,
        }),
    }
}

#[derive(Clone)]
enum SquashOperation {
    SquashCommits(SquashCommitsOperation),
    SquashBranch(SquashBranchOperation),
}

#[derive(Clone)]
struct SquashCommitsOperation {
    sources: Vec<gix::ObjectId>,
    target: gix::ObjectId,
    how_to_combine_messages: Option<MessageCombinationStrategy>,
}

impl SquashCommitsOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        let Self {
            sources,
            target,
            how_to_combine_messages,
        } = self;
        tx.squash_commits(
            sources,
            target,
            how_to_combine_messages.unwrap_or(MessageCombinationStrategy::KeepBoth),
        )
    }
}

#[derive(Clone)]
struct SquashBranchOperation {
    sources: Vec<gix::ObjectId>,
    target: gix::ObjectId,
    how_to_combine_messages: Option<MessageCombinationStrategy>,
    branch_name: FullName,
}

impl SquashBranchOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        let Self {
            sources,
            target,
            how_to_combine_messages,
            branch_name: _,
        } = self;
        tx.squash_commits(
            sources,
            target,
            how_to_combine_messages.unwrap_or(MessageCombinationStrategy::KeepBoth),
        )
    }
}
