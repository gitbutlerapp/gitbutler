use anyhow::Context as _;
use bstr::BString;
use but_api::{WorkspaceState, json::HexHash};
use but_core::{DiffSpec, DryRun, RefMetadata, sync::RepoExclusive};
use but_ctx::Context;
use but_graph::Workspace;
use but_transaction::{DynamicOutcome, IntermediateCommitCreateResult, Transaction};
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{ObjectId, refs::FullName};
use itertools::Itertools;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliError, CliResult, CliResultExt, IdMap,
    args::{
        atoms::{BranchArg, CliIdArg, Priority, Purpose, ResolvedCliIdArg},
        squash2::Platform,
    },
    bad_input,
    command::legacy::reword2::RewordCommitOperation,
    id::{UNCOMMITTED, UncommittedHunkOrFile},
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, diff_specs::DiffSpecBuilder,
    },
};

pub enum SquashOutcome {
    Commits {
        sources: NonEmpty<gix::ObjectId>,
        target: gix::ObjectId,
        new_commit: gix::ObjectId,
    },
    Branch {
        new_commit: gix::ObjectId,
        branch_names: NonEmpty<FullName>,
    },
    Hunks {
        target: gix::ObjectId,
        new_commit: gix::ObjectId,
    },
    Uncommit {
        sources: Vec<gix::ObjectId>,
    },
    UncommitHunk {
        source: gix::ObjectId,
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
            SquashOutcome::Hunks { target, new_commit } => {
                writeln!(
                    out,
                    "Amended {} to create {}",
                    theme::Commit(target),
                    theme::Commit(new_commit)
                )?;
            }
            SquashOutcome::Uncommit { sources } => {
                let commits = sources.into_iter().map(theme::Commit).join(", ");
                writeln!(out, "Uncommitted {commits}")?;
            }
            SquashOutcome::UncommitHunk { source } => {
                writeln!(out, "Uncommitted from {}", theme::Commit(source))?;
            }
        };

        Ok(())
    }
}

impl CliOutput for SquashOutcome {
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()> {
        match self {
            SquashOutcome::Commits { new_commit, .. }
            | SquashOutcome::Branch { new_commit, .. }
            | SquashOutcome::Hunks { new_commit, .. } => {
                writeln!(out, "{new_commit}")?;
                Ok(())
            }
            SquashOutcome::Uncommit { .. } | SquashOutcome::UncommitHunk { .. } => Ok(()),
        }
    }

    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        struct Output {
            new_commit: HexHash,
        }

        match self {
            SquashOutcome::Commits { new_commit, .. }
            | SquashOutcome::Branch { new_commit, .. }
            | SquashOutcome::Hunks { new_commit, .. } => Some(Output {
                new_commit: HexHash(new_commit),
            }),
            SquashOutcome::Uncommit { .. } | SquashOutcome::UncommitHunk { .. } => None,
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

    let squash_op = resolve(ctx, guard.write_permission(), args, &id_map)?;
    let outcome = run(ctx, &mut meta, guard.write_permission(), squash_op)?;

    Ok(outcome)
}

fn resolve(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    args: Platform,
    id_map: &IdMap,
) -> CliResult<SquashOperation> {
    let Platform {
        target,
        sources,
        message,
        no_message,
        use_target_message,
        use_source_message,
    } = args;

    let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm.read_permission())?;

    let resolved_squash = if let Some(target) = target {
        let target = resolve_target(
            target,
            message,
            no_message,
            use_target_message,
            use_source_message,
            &repo,
            id_map,
        )?;
        let sources = sources
            .into_iter()
            .map(|source| {
                let id = source.resolve_in_workspace(&repo, id_map, Purpose::Source, None)?;
                Squashable::try_from_resolved_id(id)
            })
            .collect::<CliResult<Vec<_>>>()?;

        let mut commit_sources = Vec::new();
        let mut branch_sources = Vec::new();
        let mut hunk_sources = Vec::new();
        let mut uncommitted_sources = Vec::new();
        let mut committed_file_sources = Vec::new();
        for source in sources {
            match source {
                Squashable::Commit(object_id) => commit_sources.push(object_id),
                Squashable::Branch(branch_arg) => branch_sources.push(branch_arg),
                Squashable::UncommittedHunkOrFile(hunk) => hunk_sources.push(*hunk),
                Squashable::Uncommitted(zz) => uncommitted_sources.push(zz),
                Squashable::CommittedFile(committed_file) => {
                    committed_file_sources.push(committed_file)
                }
            }
        }

        match ClassifiedSquashables::try_from_sources(
            commit_sources,
            branch_sources,
            hunk_sources,
            uncommitted_sources,
            committed_file_sources,
        )? {
            ClassifiedSquashables::Commits(sources) => ResolvedSquash::Commits { target, sources },
            ClassifiedSquashables::Branches(branch_sources) => {
                resolve_squash_branch(target, branch_sources, &ws)?
            }
            ClassifiedSquashables::UncommittedHunks(source_hunks) => {
                let (target, reword) = match target {
                    SquashTarget::Commit { commit, reword } => {
                        (commit, reword.try_into_uncommitting()?)
                    }
                    SquashTarget::Uncommitted => {
                        return Err(cannot_uncommit_uncommitted_changes_error());
                    }
                };
                ResolvedSquash::UncommittedHunk(AmendUncommittedHunks {
                    target,
                    source_hunks,
                    reword,
                })
            }
            ClassifiedSquashables::Uncommitted => {
                let (target, reword) = match target {
                    SquashTarget::Commit { commit, reword } => {
                        (commit, reword.try_into_uncommitting()?)
                    }
                    SquashTarget::Uncommitted => {
                        return Err(cannot_uncommit_uncommitted_changes_error());
                    }
                };
                ResolvedSquash::Uncommitted { target, reword }
            }
            ClassifiedSquashables::CommittedFiles(committed_files) => {
                let first = committed_files.first();

                let mut source_paths = Vec::from([first.path.clone()]);
                let source = first.commit_id;
                for committed_file in committed_files.into_iter().skip(1) {
                    let CommittedFile { commit_id, path } = committed_file;

                    if source != commit_id {
                        let err = format!(
                            "All committed files must come from the same commit. Found files from {} and {}",
                            source.to_hex_with_len(7),
                            commit_id.to_hex_with_len(7),
                        );
                        return Err(bad_input(err).into());
                    }

                    source_paths.push(path);
                }

                ResolvedSquash::CommittedFiles {
                    target: MoveCommittedChangesTarget::from_squash_target(target)?,
                    source,
                    source_paths,
                }
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

                let reword =
                    resolve_reword(message, no_message, use_target_message, use_source_message);

                ResolvedSquash::Branches {
                    target,
                    reword,
                    source_commits: sources,
                    source_branches: NonEmpty::new(source_branch_name),
                    branches_to_remove: Vec::new(),
                }
            }
            _ => {
                return Err(bad_input(
                    "When --target isn't used the source must be exactly one branch",
                )
                .into());
            }
        }
    };

    let squash_op = match resolved_squash {
        ResolvedSquash::Commits { target, sources } => match target {
            SquashTarget::Commit {
                commit: target,
                reword,
            } => SquashOperation::Commits(SquashCommitsOperation {
                sources,
                target,
                reword,
            }),
            SquashTarget::Uncommitted => SquashOperation::Uncommit(UncommitOperation { sources }),
        },
        ResolvedSquash::Branches {
            target,
            reword,
            source_commits,
            source_branches,
            branches_to_remove,
        } => SquashOperation::Branch(SquashBranchOperation {
            sources: source_commits,
            target,
            reword,
            source_branches,
            branches_to_remove,
        }),
        ResolvedSquash::UncommittedHunk(amend_hunks) => {
            SquashOperation::UncommittedHunks(amend_hunks)
        }
        ResolvedSquash::Uncommitted { target, reword } => {
            SquashOperation::Uncommitted { target, reword }
        }
        ResolvedSquash::CommittedFiles {
            target,
            source,
            source_paths,
        } => match target {
            MoveCommittedChangesTarget::Commit {
                commit: target,
                reword,
            } => SquashOperation::MoveCommittedFiles {
                target,
                source,
                source_paths,
                reword,
            },
            MoveCommittedChangesTarget::Uncommitted => {
                SquashOperation::UncommitCommittedFiles(UncommitCommittedFilesOperation {
                    source,
                    source_paths,
                })
            }
        },
    };

    Ok(squash_op)
}

fn cannot_uncommit_uncommitted_changes_error() -> CliError {
    bad_input("Cannot uncommit uncommitted changes")
        .hint("When squashing uncommitted changes the --target must be a commit or a branch")
        .into()
}

enum ResolvedSquash {
    Commits {
        target: SquashTarget,
        sources: NonEmpty<ObjectId>,
    },
    Branches {
        // Branches can only be squashed into commits and not uncommitted. This is because we dont
        // currently have a transaction based API to uncommit. We need this because we also need to
        // remove the reference which should happen in a transaction.
        target: ObjectId,
        reword: HowToRewordTarget,
        source_commits: Vec<ObjectId>,
        /// The branches that we're squashing.
        ///
        /// This is just used to generate the output.
        source_branches: NonEmpty<FullName>,
        /// The branches that should be removed after squashing the commits.
        branches_to_remove: Vec<FullName>,
    },
    UncommittedHunk(AmendUncommittedHunks),
    Uncommitted {
        target: ObjectId,
        reword: HowToRewordTargetNoSource,
    },
    CommittedFiles {
        target: MoveCommittedChangesTarget,
        source: ObjectId,
        source_paths: Vec<BString>,
    },
}

#[derive(Clone)]
struct AmendUncommittedHunks {
    target: ObjectId,
    source_hunks: NonEmpty<UncommittedHunkOrFile>,
    reword: HowToRewordTargetNoSource,
}

#[derive(Debug, Clone)]
enum SquashTarget {
    Commit {
        commit: ObjectId,
        reword: HowToRewordTarget,
    },
    Uncommitted,
}

#[derive(Debug, Clone)]
enum MoveCommittedChangesTarget {
    Commit {
        commit: ObjectId,
        reword: HowToRewordTargetNoSource,
    },
    Uncommitted,
}

impl MoveCommittedChangesTarget {
    fn from_squash_target(target: SquashTarget) -> CliResult<Self> {
        match target {
            SquashTarget::Commit { commit, reword } => Ok(Self::Commit {
                commit,
                reword: reword.try_into_moving_changes()?,
            }),
            SquashTarget::Uncommitted => Ok(Self::Uncommitted),
        }
    }
}

fn resolve_target(
    id: CliIdArg,
    message: Option<Vec<String>>,
    no_message: bool,
    use_target_message: bool,
    use_source_message: bool,
    repo: &gix::Repository,
    id_map: &IdMap,
) -> CliResult<SquashTarget> {
    let target_kind_hint = "--target must be an applied commit, branch, or zz";
    let hint = format!("{}. {}", target_kind_hint, CliIdArg::TARGET_MISSING_HINT);
    match id
        .resolve_in_workspace(
            repo,
            id_map,
            Purpose::Target,
            Some(Priority::BranchAndCommit),
        )
        .with_hint(|| hint.clone())?
    {
        ResolvedCliIdArg::Commit(object_id) => {
            let reword =
                resolve_reword(message, no_message, use_target_message, use_source_message);
            Ok(SquashTarget::Commit {
                commit: object_id,
                reword,
            })
        }
        ResolvedCliIdArg::Branch(branch_arg) => {
            let branch_name = branch_arg.resolve_local_branch_name()?;

            for stack in id_map.stacks() {
                for segment in &stack.segments {
                    let Some(ref_info) = &segment.inner.ref_info else {
                        continue;
                    };

                    if ref_info.ref_name == branch_name {
                        return if let Some(commit) = ref_info.commit_id {
                            let reword = resolve_reword(
                                message,
                                no_message,
                                use_target_message,
                                use_source_message,
                            );
                            Ok(SquashTarget::Commit { commit, reword })
                        } else {
                            Err(bad_input("--target cannot be an empty branch").into())
                        };
                    }
                }
            }

            Err(bad_input("target not found").hint(hint).into())
        }
        ResolvedCliIdArg::Uncommitted => {
            if message.is_some() {
                return Err(bad_input("--message cannot be used when uncommitting").into());
            }
            if no_message {
                return Err(bad_input("--no-message cannot be used when uncommitting").into());
            }
            if use_target_message {
                return Err(
                    bad_input("--use-target-message cannot be used when uncommitting").into(),
                );
            }
            if use_source_message {
                return Err(
                    bad_input("--use-source-message cannot be used when uncommitting").into(),
                );
            }

            Ok(SquashTarget::Uncommitted)
        }
        ResolvedCliIdArg::UncommittedHunkOrFile(..)
        | ResolvedCliIdArg::CommittedFile { .. }
        | ResolvedCliIdArg::PathPrefix
        | ResolvedCliIdArg::Stack => Err(bad_input(target_kind_hint)
            .hint(CliIdArg::TARGET_MISSING_HINT)
            .into()),
    }
}

fn resolve_squash_branch(
    target: SquashTarget,
    branch_sources: NonEmpty<BranchArg>,
    ws: &Workspace,
) -> CliResult<ResolvedSquash> {
    let (target, reword) = match target {
        SquashTarget::Commit { commit, reword } => (commit, reword),
        SquashTarget::Uncommitted => {
            let err = bad_input("Cannot uncommit branches")
                .hint("When squashing a branch --target must be a commit or a branch")
                .into();
            return Err(err);
        }
    };

    let mut source_branches = Vec::<FullName>::new();
    let mut branches_to_remove = Vec::<FullName>::new();
    let mut commits_on_branch_sources = Vec::new();
    for branch_name in branch_sources {
        let (source_branch_name, mut commits_on_branch) =
            resolve_commits_on_branch(&branch_name, ws)?;

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
            branches_to_remove.push(source_branch_name.clone());
        }
        source_branches.push(source_branch_name);
    }

    let source_branches = NonEmpty::from_vec(source_branches)
        .expect("source branches is already checked to be non-empty");

    Ok(ResolvedSquash::Branches {
        target,
        source_commits: commits_on_branch_sources,
        source_branches,
        branches_to_remove,
        reword,
    })
}

fn resolve_reword(
    message: Option<Vec<String>>,
    no_message: bool,
    use_target_message: bool,
    use_source_message: bool,
) -> HowToRewordTarget {
    if use_target_message {
        HowToRewordTarget::UseTargetMessage
    } else if use_source_message {
        HowToRewordTarget::UseSourceMessage
    } else {
        HowToRewordTarget::Reword(RewordCommitOperation::resolve(no_message, message))
    }
}

#[derive(Debug, Clone)]
enum HowToRewordTarget {
    UseTargetMessage,
    UseSourceMessage,
    Reword(RewordCommitOperation),
}

impl HowToRewordTarget {
    fn will_open_editor(&self) -> bool {
        match self {
            Self::UseTargetMessage | Self::UseSourceMessage => false,
            Self::Reword(op) => op.will_open_editor(),
        }
    }

    fn how_to_combine_messages(&self) -> MessageCombinationStrategy {
        match self {
            Self::UseTargetMessage => MessageCombinationStrategy::KeepTarget,
            Self::UseSourceMessage => MessageCombinationStrategy::KeepSubject,
            Self::Reword(..) => MessageCombinationStrategy::KeepBoth,
        }
    }

    fn execute(
        self,
        commit: ObjectId,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        match self {
            Self::UseTargetMessage | Self::UseSourceMessage => Ok(commit),
            Self::Reword(reword_commit_operation) => reword_commit_operation.execute(commit, tx),
        }
    }

    fn try_into_uncommitting(self) -> CliResult<HowToRewordTargetNoSource> {
        match self {
            HowToRewordTarget::UseSourceMessage => Err(bad_input(
                "--use-source-message cannot be used when squashing uncommitted changes",
            )
            .into()),
            HowToRewordTarget::UseTargetMessage => Ok(HowToRewordTargetNoSource::UseTargetMessage),
            HowToRewordTarget::Reword(op) => Ok(HowToRewordTargetNoSource::Reword(op)),
        }
    }

    fn try_into_moving_changes(self) -> CliResult<HowToRewordTargetNoSource> {
        match self {
            HowToRewordTarget::UseSourceMessage => Err(bad_input(
                "--use-source-message cannot be used when moving committed changes",
            )
            .into()),
            HowToRewordTarget::UseTargetMessage => Ok(HowToRewordTargetNoSource::UseTargetMessage),
            HowToRewordTarget::Reword(op) => Ok(HowToRewordTargetNoSource::Reword(op)),
        }
    }
}

/// Like [`HowToRewordTarget`] except it doesn't allow picking the source message.
///
/// Used when the source is uncommitted, which doesn't have messages.
#[derive(Debug, Clone)]
enum HowToRewordTargetNoSource {
    UseTargetMessage,
    Reword(RewordCommitOperation),
}

impl HowToRewordTargetNoSource {
    fn will_open_editor(&self) -> bool {
        match self {
            Self::UseTargetMessage => false,
            Self::Reword(op) => op.will_open_editor(),
        }
    }

    fn execute(
        self,
        commit: ObjectId,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        match self {
            Self::UseTargetMessage => Ok(commit),
            Self::Reword(reword_commit_operation) => reword_commit_operation.execute(commit, tx),
        }
    }
}

enum Squashable {
    Commit(gix::ObjectId),
    Branch(BranchArg),
    UncommittedHunkOrFile(Box<UncommittedHunkOrFile>),
    Uncommitted(&'static str),
    CommittedFile(CommittedFile),
}

struct CommittedFile {
    commit_id: gix::ObjectId,
    path: BString,
}

impl Squashable {
    fn try_from_resolved_id(id: ResolvedCliIdArg) -> CliResult<Self> {
        let kind = match id {
            ResolvedCliIdArg::Commit(commit) => return Ok(Self::Commit(commit)),
            ResolvedCliIdArg::Branch(branch) => return Ok(Self::Branch(branch)),
            ResolvedCliIdArg::UncommittedHunkOrFile(hunk) => {
                return Ok(Self::UncommittedHunkOrFile(hunk));
            }
            ResolvedCliIdArg::Uncommitted => return Ok(Self::Uncommitted(UNCOMMITTED)),
            ResolvedCliIdArg::CommittedFile {
                commit_id,
                path,
                id: _,
            } => {
                return Ok(Self::CommittedFile(CommittedFile { commit_id, path }));
            }
            ResolvedCliIdArg::PathPrefix => "a path",
            ResolvedCliIdArg::Stack => "a stack",
        };
        Err(bad_input(format!(
            "Expected a commit, a branch, or an uncommitted change, got {kind}"
        ))
        .into())
    }
}

enum ClassifiedSquashables {
    Commits(NonEmpty<gix::ObjectId>),
    Branches(NonEmpty<BranchArg>),
    UncommittedHunks(NonEmpty<UncommittedHunkOrFile>),
    Uncommitted,
    CommittedFiles(NonEmpty<CommittedFile>),
}

impl ClassifiedSquashables {
    fn try_from_sources(
        commit_sources: Vec<ObjectId>,
        branch_sources: Vec<BranchArg>,
        hunk_sources: Vec<UncommittedHunkOrFile>,
        uncommitted_sources: Vec<&'static str>,
        committed_file_sources: Vec<CommittedFile>,
    ) -> CliResult<Self> {
        let has_commits = !commit_sources.is_empty();
        let has_branches = !branch_sources.is_empty();
        let has_hunks = !hunk_sources.is_empty();
        let has_uncommitted = !uncommitted_sources.is_empty();
        let has_committed_file_sources = !committed_file_sources.is_empty();

        let source_type_count = [
            has_commits,
            has_branches,
            has_hunks,
            has_uncommitted,
            has_committed_file_sources,
        ]
        .into_iter()
        .filter(|has_source| *has_source)
        .count();

        if source_type_count > 1 {
            return Err(bad_input("Cannot mix different types of sources").into());
        }

        if let Some(commit_sources) = NonEmpty::from_vec(commit_sources) {
            Ok(Self::Commits(commit_sources))
        } else if let Some(branch_sources) = NonEmpty::from_vec(branch_sources) {
            Ok(Self::Branches(branch_sources))
        } else if let Some(hunk_sources) = NonEmpty::from_vec(hunk_sources) {
            Ok(Self::UncommittedHunks(hunk_sources))
        } else if has_uncommitted {
            Ok(Self::Uncommitted)
        } else if let Some(committed_file_sources) = NonEmpty::from_vec(committed_file_sources) {
            Ok(Self::CommittedFiles(committed_file_sources))
        } else {
            unreachable!(
                "`sources` is required in `Platform` so we'll never get here with no sources"
            )
        }
    }
}

fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    squash_op: SquashOperation,
) -> anyhow::Result<SquashOutcome> {
    let executable_op = match squash_op {
        SquashOperation::Commits(SquashCommitsOperation {
            mut sources,
            target,
            reword,
        }) => {
            sources = non_empty_dedup_maintain_sort(sources);

            ExecutableSquashOperation::TransactionCompatible(
                TransactionCompatibleOperation::Commits(SquashCommitsOperation {
                    sources,
                    target,
                    reword,
                }),
            )
        }
        SquashOperation::Branch(SquashBranchOperation {
            mut sources,
            mut source_branches,
            mut branches_to_remove,
            target,
            reword,
        }) => {
            sources.sort();
            sources.dedup();

            branches_to_remove.sort();
            branches_to_remove.dedup();

            source_branches = non_empty_dedup_maintain_sort(source_branches);

            ExecutableSquashOperation::TransactionCompatible(
                TransactionCompatibleOperation::Branch(SquashBranchOperation {
                    sources,
                    source_branches,
                    branches_to_remove,
                    target,
                    reword,
                }),
            )
        }
        SquashOperation::UncommittedHunks(AmendUncommittedHunks {
            target,
            source_hunks,
            reword,
        }) => {
            let context_lines = ctx.settings.context_lines;
            let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
            for hunk in &source_hunks {
                builder.push_changes_from_uncommitted(hunk)?;
            }
            builder.reconcile_worktree_diff_specs()?;
            let changes = builder.into_diff_specs();

            ExecutableSquashOperation::TransactionCompatible(
                TransactionCompatibleOperation::UncommittedHunks(
                    AmendUncommittedDiffSpecsOperation {
                        target,
                        changes,
                        reword,
                    },
                ),
            )
        }
        SquashOperation::Uncommitted { target, reword } => {
            let context_lines = ctx.settings.context_lines;
            let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
            builder.push_changes_from_uncommitted_area()?;
            let changes = builder.into_diff_specs();

            ExecutableSquashOperation::TransactionCompatible(
                TransactionCompatibleOperation::UncommittedHunks(
                    AmendUncommittedDiffSpecsOperation {
                        target,
                        changes,
                        reword,
                    },
                ),
            )
        }
        SquashOperation::MoveCommittedFiles {
            target,
            source,
            source_paths,
            reword,
        } => {
            let context_lines = ctx.settings.context_lines;
            let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
            for path in source_paths {
                builder.push_changes_from_committed_file(source, path.as_ref())?;
            }
            let changes = builder.into_diff_specs();
            ExecutableSquashOperation::TransactionCompatible(
                TransactionCompatibleOperation::MoveCommittedFiles(MoveCommittedFilesOperation {
                    target,
                    source,
                    changes,
                    reword,
                }),
            )
        }
        SquashOperation::Uncommit(UncommitOperation { mut sources }) => {
            sources = non_empty_dedup_maintain_sort(sources);
            ExecutableSquashOperation::Uncommit(UncommitOperation { sources })
        }
        SquashOperation::UncommitCommittedFiles(UncommitCommittedFilesOperation {
            source,
            source_paths,
        }) => {
            let context_lines = ctx.settings.context_lines;
            let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
            for path in source_paths {
                builder.push_changes_from_committed_file(source, path.as_ref())?;
            }
            let changes = builder.into_diff_specs();

            ExecutableSquashOperation::UncommitHunks { source, changes }
        }
    };

    match executable_op {
        ExecutableSquashOperation::TransactionCompatible(op) => {
            let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);
            let result = but_transaction::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
                DryRun::No,
                |mut tx| {
                    let new_commit = match op.clone() {
                        TransactionCompatibleOperation::Commits(op) => op.execute(&mut tx)?,
                        TransactionCompatibleOperation::Branch(op) => op.execute(&mut tx)?,
                        TransactionCompatibleOperation::UncommittedHunks(op) => {
                            op.execute(&mut tx)?
                        }
                        TransactionCompatibleOperation::MoveCommittedFiles(op) => {
                            op.execute(&mut tx)?
                        }
                    };

                    Ok(DynamicOutcome::<_, std::convert::Infallible>::Commit(
                        new_commit,
                    ))
                },
            )?;

            let DynamicOutcome::Commit((new_commit, _ws)) = result;

            match op.clone() {
                TransactionCompatibleOperation::Commits(SquashCommitsOperation {
                    sources,
                    target,
                    ..
                }) => Ok(SquashOutcome::Commits {
                    new_commit,
                    sources,
                    target,
                }),
                TransactionCompatibleOperation::Branch(SquashBranchOperation {
                    source_branches,
                    ..
                }) => Ok(SquashOutcome::Branch {
                    new_commit,
                    branch_names: source_branches,
                }),
                TransactionCompatibleOperation::UncommittedHunks(
                    AmendUncommittedDiffSpecsOperation { target, .. },
                )
                | TransactionCompatibleOperation::MoveCommittedFiles(
                    MoveCommittedFilesOperation { target, .. },
                ) => Ok(SquashOutcome::Hunks { target, new_commit }),
            }
        }
        ExecutableSquashOperation::Uncommit(op) => {
            let UncommitOperation { sources } = op;

            {
                let but_api::commit::types::UncommitResult {
                    workspace,
                    uncommitted_ids: _,
                } = but_api::commit::uncommit::commit_uncommit_only_with_perm(
                    ctx,
                    sources.iter().copied().collect(),
                    None,
                    DryRun::Yes,
                    perm,
                )?;

                anyhow::ensure!(
                    !is_workspace_conflicted(&workspace),
                    "Cannot uncommit commits that would result in merge conflicts"
                );
            }

            let but_api::commit::types::UncommitResult {
                uncommitted_ids,
                workspace: _,
            } = but_api::commit::uncommit::commit_uncommit_with_perm(
                ctx,
                sources.into_iter().collect(),
                None,
                DryRun::No,
                perm,
            )?;

            Ok(SquashOutcome::Uncommit {
                sources: uncommitted_ids,
            })
        }
        ExecutableSquashOperation::UncommitHunks { source, changes } => {
            {
                let but_api::commit::types::MoveChangesResult { workspace } =
                    but_api::commit::uncommit::commit_uncommit_changes_only_with_perm(
                        ctx,
                        source,
                        changes.clone(),
                        None,
                        DryRun::Yes,
                        perm,
                    )?;

                anyhow::ensure!(
                    !is_workspace_conflicted(&workspace),
                    "Cannot uncommit hunks that would result in merge conflicts"
                );
            }

            let but_api::commit::types::MoveChangesResult { workspace: _ } =
                but_api::commit::uncommit::commit_uncommit_changes_with_perm(
                    ctx,
                    source,
                    changes,
                    None,
                    DryRun::No,
                    perm,
                )?;

            Ok(SquashOutcome::UncommitHunk { source })
        }
    }
}

#[derive(Clone)]
enum SquashOperation {
    Commits(SquashCommitsOperation),
    Branch(SquashBranchOperation),
    UncommittedHunks(AmendUncommittedHunks),
    Uncommitted {
        target: ObjectId,
        reword: HowToRewordTargetNoSource,
    },
    MoveCommittedFiles {
        target: ObjectId,
        source: ObjectId,
        source_paths: Vec<BString>,
        reword: HowToRewordTargetNoSource,
    },
    Uncommit(UncommitOperation),
    UncommitCommittedFiles(UncommitCommittedFilesOperation),
}

impl SquashOperation {
    #[expect(dead_code)]
    pub fn will_open_editor(&self) -> bool {
        match self {
            SquashOperation::Commits(op) => op.reword.will_open_editor(),
            SquashOperation::Branch(op) => op.reword.will_open_editor(),
            SquashOperation::UncommittedHunks(op) => op.reword.will_open_editor(),
            SquashOperation::Uncommitted { reword, .. } => reword.will_open_editor(),
            SquashOperation::MoveCommittedFiles { reword, .. } => reword.will_open_editor(),
            SquashOperation::Uncommit(..) | SquashOperation::UncommitCommittedFiles(..) => false,
        }
    }
}

#[derive(Clone)]
enum ExecutableSquashOperation {
    TransactionCompatible(TransactionCompatibleOperation),
    // Unfortunately uncommitting is currently not supported by but-transaction and thus requires
    // special handling
    Uncommit(UncommitOperation),
    UncommitHunks {
        source: ObjectId,
        changes: Vec<DiffSpec>,
    },
}

#[derive(Clone)]
enum TransactionCompatibleOperation {
    Commits(SquashCommitsOperation),
    Branch(SquashBranchOperation),
    UncommittedHunks(AmendUncommittedDiffSpecsOperation),
    MoveCommittedFiles(MoveCommittedFilesOperation),
}

#[derive(Clone)]
struct SquashCommitsOperation {
    sources: NonEmpty<gix::ObjectId>,
    target: gix::ObjectId,
    reword: HowToRewordTarget,
}

impl SquashCommitsOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        let Self {
            sources,
            target,
            reword,
        } = self;
        let new_commit = tx.squash_commits(sources, target, reword.how_to_combine_messages())?;
        reword.execute(new_commit, tx)
    }
}

#[derive(Clone)]
struct SquashBranchOperation {
    sources: Vec<gix::ObjectId>,
    target: gix::ObjectId,
    reword: HowToRewordTarget,
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
            reword,
            source_branches: _,
            branches_to_remove,
        } = self;

        for branch_name in branches_to_remove {
            tx.remove_reference(branch_name.as_ref())?;
        }

        let new_commit = tx.squash_commits(sources, target, reword.how_to_combine_messages())?;
        reword.execute(new_commit, tx)
    }
}

#[derive(Clone)]
struct AmendUncommittedDiffSpecsOperation {
    target: ObjectId,
    changes: Vec<DiffSpec>,
    reword: HowToRewordTargetNoSource,
}

impl AmendUncommittedDiffSpecsOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        let Self {
            target,
            changes,
            reword,
        } = self;

        let IntermediateCommitCreateResult {
            new_commit,
            rejected_specs,
        } = tx.amend_commit(target, changes)?;

        anyhow::ensure!(rejected_specs.is_empty(), "Couldn't squash all changes");

        let new_commit =
            new_commit.context("BUG: rejected_specs is empty yet nothing was committed")?;

        reword.execute(new_commit, tx)
    }
}

#[derive(Clone)]
struct MoveCommittedFilesOperation {
    target: ObjectId,
    source: ObjectId,
    changes: Vec<but_core::DiffSpec>,
    reword: HowToRewordTargetNoSource,
}

impl MoveCommittedFilesOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        let Self {
            target,
            source,
            changes,
            reword,
        } = self;

        let new_commit = tx.move_committed_changes_between(source, target, changes)?;
        reword.execute(new_commit, tx)
    }
}

#[derive(Clone)]
struct UncommitOperation {
    sources: NonEmpty<gix::ObjectId>,
}

#[derive(Clone)]
struct UncommitCommittedFilesOperation {
    source: ObjectId,
    source_paths: Vec<BString>,
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

fn is_workspace_conflicted(workspace: &WorkspaceState) -> bool {
    workspace
        .head_info
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .flat_map(|segment| &segment.commits)
        .any(|commit| commit.has_conflicts)
}
