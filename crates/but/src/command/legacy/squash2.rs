use but_api::json::HexHash;
use but_core::{DryRun, RefMetadata, sync::RepoExclusive};
use but_ctx::Context;
use but_graph::Workspace;
use but_transaction::{DynamicOutcome, Transaction};
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{ObjectId, refs::FullName};
use itertools::Itertools;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliResult, CliResultExt, IdMap,
    args::{
        atoms::{BranchArg, BranchOrCommit, Purpose},
        squash2::Platform,
    },
    bad_input,
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
        branch_names: NonEmpty<FullName>,
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
                branch_names,
            } => {
                if branch_names.len() == 1 {
                    writeln!(
                        out,
                        "Squashed branch {} to create commit {}",
                        theme::Branch(&branch_names[0]),
                        theme::Commit(new_commit)
                    )?;
                } else {
                    let branch_names = branch_names.into_iter().map(theme::Branch).join(", ");
                    writeln!(
                        out,
                        "Squashed branches {} to create commit {}",
                        branch_names,
                        theme::Commit(new_commit)
                    )?;
                }
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

    let (target, sources, branch_resolution) = if let Some(target) = target {
        let target = target
            .resolve_commit_in_workspace(&repo, id_map)
            .hint("--target must always target a commit on an applied branch")?;
        let sources = sources
            .into_iter()
            .map(|source| {
                source
                    .resolve_in_workspace(&repo, id_map, Purpose::Source, None)?
                    .into_branch_or_commit()
            })
            .collect::<CliResult<Vec<_>>>()?;

        let mut commit_sources = Vec::new();
        let mut branch_sources = Vec::new();
        for source in sources {
            match source {
                BranchOrCommit::Commit(object_id) => commit_sources.push(object_id),
                BranchOrCommit::Branch(branch_arg) => branch_sources.push(branch_arg),
            }
        }

        match (commit_sources.is_empty(), branch_sources.is_empty()) {
            (true, true) => {
                // nothing to squash
                unreachable!(
                    "`sources` is required in `Platform` so we'll never get here with no commits and no branches"
                )
            }
            (true, false) => {
                // squash only branches
                let mut sources = Vec::<FullName>::new();
                let mut to_remove = Vec::<FullName>::new();
                let mut commits_on_branch_sources = Vec::new();
                for branch_name in branch_sources {
                    let (source_branch_name, mut commits_on_branch) =
                        resolve_commits_on_branch(&branch_name, &ws)?;

                    let mut target_commit_exists_on_branch = false;
                    commits_on_branch.retain(|commit| {
                        if *commit == target {
                            target_commit_exists_on_branch = true;
                            false
                        } else {
                            true
                        }
                    });
                    commits_on_branch_sources.append(&mut commits_on_branch);

                    if !target_commit_exists_on_branch {
                        to_remove.push(source_branch_name.clone());
                    }
                    sources.push(source_branch_name);
                }

                let sources = NonEmpty::from_vec(sources)
                    .expect("source branches is already checked to be non-empty");

                (
                    target,
                    commits_on_branch_sources,
                    Some(BranchResolution { sources, to_remove }),
                )
            }
            (false, true) => {
                // squash only commits
                (target, commit_sources, None)
            }
            (false, false) => {
                // mixed sources
                return Err(bad_input(
                    "Cannot mix different types of sources. Got both branches and commits",
                )
                .into());
            }
        }
    } else {
        match &sources[..] {
            [source] => {
                let branch = source.resolve_branch_in_workspace(&repo, id_map)?;
                let (source_branch_name, mut sources) = resolve_commits_on_branch(&branch, &ws)?;
                let Some(target) = sources.pop() else {
                    return Err(bad_input("Cannot squash empty branch into itself").into());
                };

                (
                    target,
                    sources,
                    Some(BranchResolution {
                        sources: NonEmpty::new(source_branch_name),
                        to_remove: Vec::new(),
                    }),
                )
            }
            _ => {
                return Err(bad_input(
                    "When --target isn't used the source must be exactly one branch",
                )
                .into());
            }
        }
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

    let squash_op = match branch_resolution {
        Some(BranchResolution {
            sources: source_branches,
            to_remove: branches_to_remove,
        }) => SquashOperation::Branch(SquashBranchOperation {
            sources,
            target,
            how_to_combine_messages,
            source_branches,
            branches_to_remove,
        }),
        None => SquashOperation::Commits(SquashCommitsOperation {
            sources,
            target,
            how_to_combine_messages,
        }),
    };

    Ok((squash_op, reword_op))
}

/// What should happen to the branch after squashing?
struct BranchResolution {
    /// The branches that we're squashing.
    ///
    /// This is just used to generate the output.
    sources: NonEmpty<FullName>,
    /// The branches that should be removed after squashing the commits.
    to_remove: Vec<FullName>,
}

fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    mut squash_op: SquashOperation,
    reword_op: Option<RewordCommitOperation>,
) -> CliResult<SquashOutcome> {
    squash_op = match squash_op {
        SquashOperation::Commits(SquashCommitsOperation {
            mut sources,
            target,
            how_to_combine_messages,
        }) => {
            sources.sort();
            sources.dedup();

            SquashOperation::Commits(SquashCommitsOperation {
                sources,
                target,
                how_to_combine_messages,
            })
        }
        SquashOperation::Branch(SquashBranchOperation {
            mut sources,
            mut source_branches,
            mut branches_to_remove,
            target,
            how_to_combine_messages,
        }) => {
            sources.sort();
            sources.dedup();

            branches_to_remove.sort();
            branches_to_remove.dedup();

            source_branches = non_empty_dedup_maintain_sort(source_branches);

            SquashOperation::Branch(SquashBranchOperation {
                sources,
                source_branches,
                branches_to_remove,
                target,
                how_to_combine_messages,
            })
        }
    };

    let (sources, target, branch_names) = match squash_op.clone() {
        SquashOperation::Commits(SquashCommitsOperation {
            sources,
            target,
            how_to_combine_messages: _,
        }) => (sources, target, None),
        SquashOperation::Branch(SquashBranchOperation {
            sources,
            target,
            source_branches,
            how_to_combine_messages: _,
            branches_to_remove: _,
        }) => (sources, target, Some(source_branches)),
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
                SquashOperation::Commits(op) => op.execute(&mut tx)?,
                SquashOperation::Branch(op) => op.execute(&mut tx)?,
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

    match branch_names {
        Some(branch_names) => Ok(SquashOutcome::Branch {
            new_commit,
            branch_names,
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
    Commits(SquashCommitsOperation),
    Branch(SquashBranchOperation),
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
    source_branches: NonEmpty<FullName>,
    branches_to_remove: Vec<FullName>,
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
            source_branches: _,
            branches_to_remove,
        } = self;

        for branch_name in branches_to_remove {
            tx.remove_reference(branch_name.as_ref())?;
        }

        tx.squash_commits(
            sources,
            target,
            how_to_combine_messages.unwrap_or(MessageCombinationStrategy::KeepBoth),
        )
    }
}

fn resolve_commits_on_branch(
    branch: &BranchArg,
    ws: &Workspace,
) -> CliResult<(FullName, Vec<ObjectId>)> {
    let branch_name = branch.resolve_local_branch_name()?;
    let (_, segment) = ws.try_find_segment_and_stack_by_refname(branch_name.as_ref())?;
    let commits_in_segment = segment.commits.iter().map(|commit| commit.id).collect();
    Ok((branch_name, commits_in_segment))
}

fn non_empty_dedup_maintain_sort<T>(non_empty: NonEmpty<T>) -> NonEmpty<T>
where
    T: Ord,
{
    let mut out = Vec::new();
    for item in non_empty {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    NonEmpty::from_vec(out).expect("deduping a NonEmpty will never make it empty")
}
