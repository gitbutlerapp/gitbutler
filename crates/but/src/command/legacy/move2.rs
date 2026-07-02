use std::fmt::Display;

use bstr::{BString, ByteSlice};
use but_core::{DiffSpec, DryRun, RefMetadata, ref_metadata::StackId, sync::RepoExclusive};
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
        atoms::{BranchArg, BranchOrCommit, CliIdArg, Purpose, ResolvedCliIdArg},
        move2::Platform,
    },
    bad_input,
    theme::{self, Theme},
    utils::{
        CliOutputHuman, IntermediateChannel, WriteWithUtils, diff_specs::DiffSpecBuilder,
        targeting::Side,
    },
};

pub enum MoveOutcome {
    Commits {
        sources: NonEmpty<gix::ObjectId>,
        target: Option<MoveTarget>,
        new_branch_name: Option<FullName>,
    },
    Changes {
        source_commit_id: gix::ObjectId,
        num_changes: usize,
        target: Option<MoveTarget>,
        new_branch_name: Option<FullName>,
        new_commit_id: gix::ObjectId,
    },
    StackBranch {
        source_branch: FullName,
        target_branch: FullName,
    },
    UnstackBranch {
        source_branch: FullName,
    },
}

impl CliOutputHuman for MoveOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _theme: &Theme) -> anyhow::Result<()> {
        match self {
            Self::Commits {
                sources,
                target,
                new_branch_name,
            } => {
                let sources = sources.into_iter().map(theme::Commit).join(", ");
                write!(out, "Moved {sources}")?;
                if let Some(new_branch_name) = new_branch_name {
                    write!(out, " to new branch {}", theme::Branch(new_branch_name))?;
                }

                if let Some(target) = target {
                    write!(out, " {target}")?;
                }
            }
            Self::Changes {
                source_commit_id,
                num_changes,
                target,
                new_branch_name,
                new_commit_id,
            } => {
                write!(
                    out,
                    "Moved {} changes from {} to new commit {}",
                    num_changes,
                    theme::Commit(source_commit_id),
                    theme::Commit(new_commit_id),
                )?;

                if let Some(new_branch_name) = new_branch_name {
                    write!(out, " on new branch {}", theme::Branch(new_branch_name))?;
                }

                if let Some(target) = target {
                    write!(out, " {target}")?;
                }
            }
            Self::StackBranch {
                source_branch,
                target_branch,
            } => {
                write!(
                    out,
                    "Stacked branch {} on top of branch {}",
                    theme::Branch(source_branch),
                    theme::Branch(target_branch),
                )?;
            }
            Self::UnstackBranch { source_branch } => {
                write!(out, "Unstacked branch {}", theme::Branch(source_branch))?;
            }
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
    CommitsRelativeTo(MoveCommitsRelativeToOperation),
    CommitsToNewBranch(MoveCommitsToNewBranchOperation),
    ChangesRelativeTo(MoveChangesRelativeToOperation),
    ChangesToNewBranch(MoveChangesToNewBranchOperation),
    StackBranch(StackBranchOnOperation),
    UnstackBranch(UnstackBranchOperation),
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
struct MoveChangesRelativeToOperation {
    source_commit_id: gix::ObjectId,
    changes: NonEmpty<DiffSpec>,
    target: MoveTarget,
}

impl MoveChangesRelativeToOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<(gix::ObjectId, Option<FullName>)> {
        let Self {
            target,
            changes,
            source_commit_id,
        } = self;

        let (relative_to, side, new_branch_name) = match target {
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

        let empty_commit_id = tx.insert_blank_commit(relative_to, side)?;
        let new_commit_id =
            tx.move_committed_changes_between(source_commit_id, empty_commit_id, changes.into())?;

        Ok((new_commit_id, new_branch_name))
    }
}

#[derive(Clone)]
struct MoveChangesToNewBranchOperation {
    source_commit_id: gix::ObjectId,
    changes: NonEmpty<DiffSpec>,
    branch_name: Option<FullName>,
}

impl MoveChangesToNewBranchOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<(gix::ObjectId, FullName)> {
        let Self {
            source_commit_id,
            changes,
            branch_name,
        } = self;

        let new_branch_name = if let Some(branch_name) = branch_name {
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

        let empty_commit_id = tx.insert_blank_commit(
            RelativeTo::Reference(new_branch_name.clone()),
            Side::Below.into(),
        )?;
        let new_commit_id =
            tx.move_committed_changes_between(source_commit_id, empty_commit_id, changes.into())?;
        Ok((new_commit_id, new_branch_name))
    }
}

#[derive(Clone)]
struct StackBranchOnOperation {
    source_branch: FullName,
    target_branch: FullName,
}

impl StackBranchOnOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<()> {
        tx.stack_branch_on(self.source_branch.as_ref(), self.target_branch.as_ref())
    }
}

#[derive(Clone)]
struct UnstackBranchOperation {
    source_branch: FullName,
}

impl UnstackBranchOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<()> {
        tx.tear_off_branch(self.source_branch.as_ref())
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
        unstack,
    } = args;

    let context_lines = ctx.settings.context_lines;
    let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;

    let resolved_sources = resolve_sources(&repo, &ws, &mut db, context_lines, id_map, sources)?;

    match (branch, above, below, unstack) {
        (Some(Some(branch)), None, None, false) => {
            match (branch.try_resolve_branch(&repo, id_map)?, resolved_sources) {
                (_, ResolvedSources::Branch(_)) => {
                    Err(bad_input("Cannot combine `--branch` with a branch source").into())
                }
                (
                    Some(branch),
                    ResolvedSources::Commits {
                        resolved_commits: sources,
                        ..
                    },
                ) => Ok(MoveOperation::CommitsRelativeTo(
                    MoveCommitsRelativeToOperation {
                        sources,
                        target: MoveTarget::BranchTip {
                            name: branch.resolve_local_branch_name()?,
                        },
                    },
                )),
                (
                    None,
                    ResolvedSources::Commits {
                        resolved_commits: sources,
                        ..
                    },
                ) => {
                    let branch_name =
                        BranchArg(branch.to_string()).resolve_for_creation(&repo, &ws)?;
                    Ok(MoveOperation::CommitsToNewBranch(
                        MoveCommitsToNewBranchOperation {
                            sources,
                            branch_name: Some(branch_name),
                        },
                    ))
                }
                (Some(branch), ResolvedSources::CommittedChanges((source_commit_id, changes))) => {
                    Ok(MoveOperation::ChangesRelativeTo(
                        MoveChangesRelativeToOperation {
                            source_commit_id,
                            changes,
                            target: MoveTarget::BranchTip {
                                name: branch.resolve_local_branch_name()?,
                            },
                        },
                    ))
                }
                (None, ResolvedSources::CommittedChanges((source_commit_id, changes))) => {
                    let branch_name =
                        BranchArg(branch.to_string()).resolve_for_creation(&repo, &ws)?;
                    Ok(MoveOperation::ChangesToNewBranch(
                        MoveChangesToNewBranchOperation {
                            source_commit_id,
                            changes,
                            branch_name: Some(branch_name),
                        },
                    ))
                }
            }
        }
        (Some(None), None, None, false) => match resolved_sources {
            ResolvedSources::Branch(branch) => Err(bad_input("Invalid target for branch source")
                .arg_name("--branch")
                .hint(format!(
                    "Trying to unstack {}? Use `--unstack '{}'` instead!",
                    branch.shorten(),
                    branch.shorten()
                ))
                .into()),
            ResolvedSources::Commits {
                resolved_commits: sources,
                ..
            } => Ok(MoveOperation::CommitsToNewBranch(
                MoveCommitsToNewBranchOperation {
                    sources,
                    branch_name: None,
                },
            )),
            ResolvedSources::CommittedChanges((source_commit_id, changes)) => Ok(
                MoveOperation::ChangesToNewBranch(MoveChangesToNewBranchOperation {
                    source_commit_id,
                    changes,
                    branch_name: None,
                }),
            ),
        },
        (None, Some(above), None, false) => {
            create_move_above_or_below_op(&repo, id_map, resolved_sources, above, Side::Above)
        }
        (None, None, Some(below), false) => {
            create_move_above_or_below_op(&repo, id_map, resolved_sources, below, Side::Below)
        }
        (None, None, None, true) => match resolved_sources {
            ResolvedSources::Branch(source_branch) => {
                Ok(MoveOperation::UnstackBranch(UnstackBranchOperation {
                    source_branch,
                }))
            }
            other => Err(bad_input(format!(
                "Cannot unstack {}, only a branch source is allowed with `--unstack`",
                other.display_kind()
            ))
            .arg_name("<SOURCES>")
            .hint("Trying to move stuff to a new branch? Use the `--branch` argument instead!")
            .into()),
        },
        _ => unreachable!("BUG: Targeting group is required"),
    }
}

fn create_move_above_or_below_op(
    repo: &gix::Repository,
    id_map: &IdMap,
    resolved_sources: ResolvedSources,
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

    match resolved_sources {
        ResolvedSources::Branch(source_branch) => {
            let MoveTarget::BranchBucket {
                name: target_branch,
                side: Side::Above,
            } = target
            else {
                return Err(bad_input("Invalid target for branch source")
                    .arg_name(format!("--{side}"))
                    .arg_value(unresolved_target.to_string())
                    .hint("Branches can only be moved with `--above <branch>` to stack or `--unstack` to unstack")
                    .into());
            };

            if source_branch == target_branch {
                return Err(bad_input("Source cannot also be target")
                    .arg_name(format!("--{side}"))
                    .arg_value(unresolved_target.to_string())
                    .into());
            }

            Ok(MoveOperation::StackBranch(StackBranchOnOperation {
                source_branch,
                target_branch,
            }))
        }
        ResolvedSources::Commits {
            resolved_commits,
            args,
        } => {
            if let MoveTarget::Commit {
                commit_id: target_commit_id,
                ..
            } = &target
            {
                for (i, source_commit_id) in resolved_commits.iter().enumerate() {
                    if source_commit_id == target_commit_id {
                        let unresolved_source = args
                            .get(i)
                            .expect("BUG: No CLI argument for resolved commit id");
                        return Err(bad_input("Source cannot also be target")
                            .arg_value(unresolved_source.to_string())
                            .arg_name(format!("--{side}"))
                            .hint(format!("Trying to move items {side} '{unresolved_source}'? Remove '{unresolved_source}' from '<SOURCES>' and try again!"))
                            .into());
                    }
                }
            }

            Ok(MoveOperation::CommitsRelativeTo(
                MoveCommitsRelativeToOperation {
                    sources: resolved_commits,
                    target,
                },
            ))
        }
        ResolvedSources::CommittedChanges((source_commit_id, changes)) => Ok(
            MoveOperation::ChangesRelativeTo(MoveChangesRelativeToOperation {
                changes,
                source_commit_id,
                target,
            }),
        ),
    }
}

enum ResolvedSources {
    Commits {
        resolved_commits: NonEmpty<gix::ObjectId>,
        /// We need the original arguments for error information to users. They are in the same
        /// order as the resolved commits - access by index!
        args: NonEmpty<CliIdArg>,
    },
    CommittedChanges((gix::ObjectId, NonEmpty<DiffSpec>)),
    Branch(FullName),
}

impl ResolvedSources {
    fn display_kind(&self) -> String {
        match self {
            Self::Commits { .. } => String::from("commits"),
            Self::CommittedChanges(_) => String::from("committed changes"),
            Self::Branch(_) => String::from("a branch"),
        }
    }
}

fn resolve_sources(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    db: &mut but_db::DbHandle,
    context_lines: u32,
    id_map: &IdMap,
    sources: impl IntoIterator<Item = CliIdArg>,
) -> CliResult<ResolvedSources> {
    let mut commit_sources: Vec<gix::ObjectId> = vec![];
    let mut file_sources: Vec<(gix::ObjectId, BString)> = vec![];
    let mut branch_sources: Vec<FullName> = vec![];
    let mut args: Vec<CliIdArg> = vec![];

    for unresolved_source in sources {
        let source_str = unresolved_source.to_string();
        args.push(unresolved_source.clone());

        match unresolved_source.resolve_in_workspace(repo, id_map, Purpose::Source, None)? {
            ResolvedCliIdArg::Commit(source_commit_id) => commit_sources.push(source_commit_id),
            ResolvedCliIdArg::CommittedFile {
                commit_id,
                path,
                id: _,
            } => {
                file_sources.push((commit_id, path));
            }
            ResolvedCliIdArg::Branch(branch) => {
                branch_sources.push(branch.resolve_local_branch_name()?);
            }
            resolved => {
                return Err(bad_input(format!("Cannot pass {resolved} as source"))
                    .arg_value(source_str)
                    .arg_name("<SOURCES>")
                    .hint("A source must be commit, committed file or branch")
                    .into());
            }
        }
    }

    match (
        NonEmpty::from_vec(commit_sources),
        NonEmpty::from_vec(file_sources),
        NonEmpty::from_vec(branch_sources),
    ) {
        (Some(resolved_commits), None, None) => {
            let args = NonEmpty::from_vec(args).expect(
                "BUG: Source arguments cannot be empty if resolved arguments are non-empty",
            );

            Ok(ResolvedSources::Commits {
                resolved_commits,
                args,
            })
        }
        (None, Some(files), None) => {
            let mut builder = DiffSpecBuilder::new(db, repo, ws, context_lines);
            let source_commit_id = files.first().0;
            for (commit_id, path) in files.into_iter() {
                if commit_id != source_commit_id {
                    return Err(
                        bad_input("Cannot move changes from multiple commits")
                            .hint("Move changes from a single commit at first, then squash additional changes into the new commit")
                            .into()
                    );
                }

                builder.push_changes_from_committed_file(commit_id, path.as_bstr())?;
            }

            // It doesn't appear as if we need to sort DiffSpecs when they're resolved on a file
            // level. For the future hunk level DiffSpecs we may need to, however.
            let changes = NonEmpty::from_vec(builder.into_diff_specs())
                .expect("BUG: Cannot possibly not have any changes here");

            Ok(ResolvedSources::CommittedChanges((
                source_commit_id,
                changes,
            )))
        }
        (None, None, Some(branches)) => {
            if !branches.tail.is_empty() {
                Err(bad_input("Branches can only be moved one at a time")
                    .arg_name("<SOURCES>")
                    .into())
            } else {
                Ok(ResolvedSources::Branch(branches.head))
            }
        }
        (None, None, None) => panic!("BUG: It should not be possible to omit sources"),
        (_, _, _) => Err(bad_input("Mixing source types is not allowed")
            .hint("You can only move one kind of source (e.g. commits) at a time")
            .arg_name("<SOURCES>")
            .into()),
    }
}

fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    move_op: MoveOperation,
) -> CliResult<MoveOutcome> {
    let snapshot_details = match &move_op {
        MoveOperation::CommitsRelativeTo(_) | MoveOperation::CommitsToNewBranch(_) => {
            SnapshotDetails::new(OperationKind::MoveCommit)
        }
        MoveOperation::ChangesRelativeTo(_) | MoveOperation::ChangesToNewBranch(_) => {
            SnapshotDetails::new(OperationKind::MoveCommitFile)
        }
        MoveOperation::StackBranch(_) => SnapshotDetails::new(OperationKind::MoveBranch),
        MoveOperation::UnstackBranch(_) => SnapshotDetails::new(OperationKind::TearOffBranch),
    };
    let (outcome, _ws) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let outcome = match move_op {
                MoveOperation::CommitsRelativeTo(op) => MoveOutcome::Commits {
                    sources: op.sources.clone(),
                    target: Some(op.target.clone()),
                    new_branch_name: op.execute(&mut tx)?,
                },
                MoveOperation::CommitsToNewBranch(op) => MoveOutcome::Commits {
                    sources: op.sources.clone(),
                    target: None,
                    new_branch_name: Some(op.execute(&mut tx)?),
                },
                MoveOperation::ChangesRelativeTo(op) => {
                    let target = op.target.clone();
                    let num_changes = op.changes.len();
                    let source_commit_id = op.source_commit_id;
                    let (new_commit_id, new_branch_name) = op.execute(&mut tx)?;
                    MoveOutcome::Changes {
                        source_commit_id,
                        num_changes,
                        target: Some(target),
                        new_branch_name,
                        new_commit_id,
                    }
                }
                MoveOperation::ChangesToNewBranch(op) => {
                    let num_changes = op.changes.len();
                    let source_commit_id = op.source_commit_id;
                    let (new_commit_id, new_branch_name) = op.execute(&mut tx)?;
                    MoveOutcome::Changes {
                        source_commit_id,
                        num_changes,
                        target: None,
                        new_branch_name: Some(new_branch_name),
                        new_commit_id,
                    }
                }
                MoveOperation::StackBranch(op) => {
                    let source_branch = op.source_branch.clone();
                    let target_branch = op.target_branch.clone();
                    op.execute(&mut tx)?;
                    MoveOutcome::StackBranch {
                        source_branch,
                        target_branch,
                    }
                }
                MoveOperation::UnstackBranch(op) => {
                    let source_branch = op.source_branch.clone();
                    op.execute(&mut tx)?;
                    MoveOutcome::UnstackBranch { source_branch }
                }
            };

            Ok(but_transaction::Commit(outcome))
        },
    )?;

    Ok(outcome)
}
