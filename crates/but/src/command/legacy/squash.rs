use std::borrow::Cow;

use anyhow::Context as _;
use bstr::{BString, ByteSlice as _};
use but_api::json::{ChangeIdString, HexHash};
use but_core::{DiffSpec, DryRun, RefMetadata, sync::RepoExclusive};
use but_ctx::Context;
use but_graph::Workspace;
use but_transaction::{IntermediateCommitCreateResult, Transaction};
use but_workspace::{
    RefInfo,
    commit::{ChangeSource, squash_commits::MessageCombinationStrategy},
};
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{
    ObjectId,
    refs::{FullName, FullNameRef},
};
use itertools::{Either, Itertools};
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliError, CliResult, CliResultExt, IdMap,
    args::{
        atoms::{BranchArg, CliIdArg, Priority, Purpose, ResolvedCliIdArg, ResolvedCliIdArgRef},
        squash::Platform,
    },
    bad_input,
    command::legacy::reword2::CommitMessageSource,
    id::{CommitId, CommitIdRef, CommittedFileId, IdAndHunk, UNCOMMITTED, UncommittedHunkOrFile},
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        diff_specs::DiffSpecBuilder, merged_upstream::MergedUpstream, rejection,
    },
};

pub enum SquashOutcome {
    Commits {
        sources: NonEmpty<CommitId>,
        target: CommitId,
        new_commit: CommitId,
    },
    Branch {
        new_commit: CommitId,
        branch_names: NonEmpty<FullName>,
    },
    Hunks {
        target: CommitId,
        new_commit: CommitId,
    },
    UncommitCommit {
        sources: Vec<CommitId>,
    },
    UncommitHunk {
        source: CommitId,
    },
    UncommitBranch {
        branch_names: NonEmpty<FullName>,
    },
}

impl CliOutputHuman for SquashOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &Theme,
    ) -> anyhow::Result<()> {
        match self {
            SquashOutcome::Commits {
                sources,
                target,
                new_commit: _,
            } => {
                let sources = sources.into_iter().map(theme::Commit).join(", ");

                writeln!(out, "Squashed {} into {}", sources, theme::Commit(target),)?;
            }
            SquashOutcome::Branch {
                new_commit,
                branch_names,
            } => {
                if branch_names.len() == 1 {
                    writeln!(
                        out,
                        "Squashed branch {} into {}",
                        theme::Branch(&branch_names[0]),
                        theme::Commit(new_commit)
                    )?;
                } else {
                    let branch_names = branch_names.into_iter().map(theme::Branch).join(", ");
                    writeln!(
                        out,
                        "Squashed branches {} into {}",
                        branch_names,
                        theme::Commit(new_commit)
                    )?;
                }
            }
            SquashOutcome::Hunks {
                target,
                new_commit: _,
            } => {
                writeln!(out, "Amended {}", theme::Commit(target),)?;
            }
            SquashOutcome::UncommitCommit { sources } => {
                let commits = sources.into_iter().map(theme::Commit).join(", ");
                writeln!(out, "Uncommitted {commits}")?;
            }
            SquashOutcome::UncommitHunk { source } => {
                writeln!(out, "Uncommitted from {}", theme::Commit(source))?;
            }
            SquashOutcome::UncommitBranch { branch_names } => {
                let branches = branch_names.into_iter().map(theme::Branch).join(", ");
                writeln!(out, "Uncommitted {branches}")?;
            }
        };

        Ok(())
    }
}

impl CliOutput for SquashOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            new_commit_id: HexHash,
            #[serde(skip_serializing_if = "Option::is_none")]
            new_commit_change_id: Option<ChangeIdString>,
        }

        match self {
            SquashOutcome::Commits { new_commit, .. }
            | SquashOutcome::Branch { new_commit, .. }
            | SquashOutcome::Hunks { new_commit, .. } => Some(Output {
                new_commit_id: new_commit.commit_id.into(),
                new_commit_change_id: new_commit.change_id.map(Into::into),
            }),
            SquashOutcome::UncommitCommit { .. }
            | SquashOutcome::UncommitHunk { .. }
            | SquashOutcome::UncommitBranch { .. } => None,
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
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let head_info = but_api::legacy::workspace::head_info(ctx)?;
    let merged = MergedUpstream::new(&*ctx.repo.get()?, &head_info, args.allow_merged);

    let (repo, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    let resolved_args = resolve_args(&repo, args, &id_map, &head_info)?;
    let resolved_args = resolved_args.as_ref();

    let squash_op = resolve(resolved_args, &ws, &repo, &merged)?;

    drop(repo);
    drop(ws);

    Ok(run(ctx, &mut meta, guard.write_permission(), squash_op)?)
}

fn resolve_args(
    repo: &gix::Repository,
    args: Platform,
    id_map: &IdMap,
    head_info: &RefInfo,
) -> CliResult<ResolvedSquashArgs> {
    let Platform {
        target,
        sources,
        message,
        no_message,
        use_target_message,
        use_source_message,
        // Consumed by the caller when building the `MergedUpstream` guard.
        allow_merged: _,
    } = args;

    let reword = resolve_reword(message, no_message, use_target_message, use_source_message)?;

    if let Some(target) = target {
        let resolved_sources = if sources.is_empty() {
            Vec::from([ResolvedCliIdArg::Uncommitted])
        } else {
            sources
                .iter()
                .map(|source| source.resolve_in_workspace(repo, id_map, Purpose::Source, None))
                .collect::<CliResult<Vec<_>>>()?
        };

        let target_kind_hint = "--target must be an applied commit, branch, or zz";
        let hint = format!("{}. {}", target_kind_hint, CliIdArg::TARGET_MISSING_HINT);
        let resolved_target = target
            .resolve_in_workspace(
                repo,
                id_map,
                Purpose::Target,
                Some(Priority::BranchAndCommit),
            )
            .with_hint(|| hint.clone())?;

        let target = match resolve_target(resolved_target.as_ref(), reword, head_info, repo) {
            Ok(target) => target,
            Err(err) => {
                return Err(match err {
                    ResolveTargetError::CannotBeEmptyBranch => {
                        bad_input("--target cannot be an empty branch").into()
                    }
                    ResolveTargetError::NotFound => bad_input("target not found").hint(hint).into(),
                    ResolveTargetError::UseTargetMessageUnavailable => {
                        bad_input("--use-target-message cannot be used when uncommitting").into()
                    }
                    ResolveTargetError::UseSourceMessageUnavailable => {
                        bad_input("--use-source-message cannot be used when uncommitting").into()
                    }
                    ResolveTargetError::NoMessageUnavailable => {
                        bad_input("--no-message cannot be used when uncommitting").into()
                    }
                    ResolveTargetError::MessageUnavailable => {
                        bad_input("--message cannot be used when uncommitting").into()
                    }
                    ResolveTargetError::InvalidTarget => bad_input(target_kind_hint)
                        .hint(CliIdArg::TARGET_MISSING_HINT)
                        .into(),
                    ResolveTargetError::Other(err) => err.into(),
                });
            }
        };

        Ok(ResolvedSquashArgs::Normal {
            sources: resolved_sources,
            target,
        })
    } else {
        match &sources[..] {
            [source] => {
                let branch = source.resolve_branch_in_workspace(repo, id_map)?;
                Ok(ResolvedSquashArgs::SingleBranchSourceAndTarget { branch, reword })
            }
            _ => {
                let mut err =
                    bad_input("When --target isn't used the source must be exactly one branch");
                // The retired `but squash <id>...` form squashed everything
                // into the last ID; suggest the flagged equivalent when the
                // sources can be re-emitted verbatim without being re-parsed
                // as flags.
                if let Some((last, rest)) = sources.split_last()
                    && !rest.is_empty()
                    && sources
                        .iter()
                        .all(|source| crate::retired_syntax::plain(&source.0))
                {
                    let rest: Vec<&str> = rest.iter().map(|source| source.0.as_str()).collect();
                    err = err.hint(format!(
                        "To squash into the last source, use `but squash {} -t {}`",
                        rest.join(" "),
                        last.0
                    ));
                }
                Err(err.into())
            }
        }
    }
}

/// [`resolve`] does a lot of work that we don't wanna duplicate in the TUI but the TUI also
/// shouldn't have to pass [`Platform`] to [`resolve`]. Hence this "pre-resolved" type exists to
/// give clients a bit more flexibility.
///
/// The CLI calls [`resolve_args`] to get `ResolvedCliIdArg` whereas the TUI builds it directly.
///
/// Note the TUI actually uses [`ResolvedSquashArgsRef`] to avoid potentially expensive clones but
/// the serves the same purpose.
enum ResolvedSquashArgs {
    /// The normal squash flow where we squash a list of sources into a target.
    Normal {
        sources: Vec<ResolvedCliIdArg>,
        target: SquashTarget,
    },
    /// The special flow where we squash a single branch into the bottom most commit in that
    /// branch. This corresponds to `but squash some-branch`.
    SingleBranchSourceAndTarget {
        branch: BranchArg,
        reword: HowToRewordTarget,
    },
}

impl ResolvedSquashArgs {
    fn as_ref(&self) -> ResolvedSquashArgsRef<'_> {
        match self {
            ResolvedSquashArgs::Normal { sources, target } => ResolvedSquashArgsRef::Normal {
                sources: sources.iter().map(|s| s.as_ref()).collect(),
                target: target.clone(),
            },
            ResolvedSquashArgs::SingleBranchSourceAndTarget { branch, reword } => {
                ResolvedSquashArgsRef::SingleBranchSourceAndTarget {
                    branch: branch.clone(),
                    reword: reword.clone(),
                }
            }
        }
    }
}

/// See [`ResolvedSquashArgs`].
pub enum ResolvedSquashArgsRef<'a> {
    Normal {
        sources: Vec<ResolvedCliIdArgRef<'a>>,
        target: SquashTarget,
    },
    SingleBranchSourceAndTarget {
        branch: BranchArg,
        reword: HowToRewordTarget,
    },
}

pub fn resolve<'a>(
    args: ResolvedSquashArgsRef<'a>,
    ws: &Workspace,
    repo: &gix::Repository,
    merged: &MergedUpstream,
) -> CliResult<SquashOperation<'a>> {
    let resolved_squash = match args {
        ResolvedSquashArgsRef::Normal { sources, target } => {
            let sources = sources
                .iter()
                .map(|source| Squashable::try_from_resolved_id(*source))
                .collect::<CliResult<Vec<_>>>()?;

            let mut commit_sources = Vec::new();
            let mut branch_sources = Vec::new();
            let mut uncommitted_change_sources = Vec::new();
            let mut uncommitted_sources = Vec::new();
            let mut committed_file_sources = Vec::new();
            for source in sources {
                match source {
                    Squashable::Commit(object_id) => commit_sources.push(object_id),
                    Squashable::Branch(branch_arg) => branch_sources.push(branch_arg),
                    Squashable::UncommittedChange(change) => {
                        uncommitted_change_sources.push(change)
                    }
                    Squashable::Uncommitted(zz) => uncommitted_sources.push(zz),
                    Squashable::CommittedFile(committed_file) => {
                        committed_file_sources.push(committed_file)
                    }
                }
            }

            match ClassifiedSquashables::try_from_sources(
                commit_sources,
                branch_sources,
                uncommitted_change_sources,
                uncommitted_sources,
                committed_file_sources,
            )? {
                ClassifiedSquashables::Commits(sources) => {
                    ResolvedSquash::Commits { target, sources }
                }
                ClassifiedSquashables::Branches(branch_sources) => {
                    resolve_squash_branch(target, branch_sources, repo, ws)?
                }
                ClassifiedSquashables::UncommittedChanges(sources) => {
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
                        sources,
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
                    let first = committed_files.head.clone();

                    let mut source_paths = Vec::from([first.path.clone()]);
                    let source = first.commit_id;
                    for committed_file in committed_files.into_iter().skip(1) {
                        let CommittedFileId {
                            commit_id,
                            path,
                            change_id: _,
                        } = committed_file;

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
                        source: CommitId {
                            commit_id: first.commit_id,
                            change_id: first.change_id.clone(),
                        },
                        source_paths,
                    }
                }
            }
        }
        ResolvedSquashArgsRef::SingleBranchSourceAndTarget { branch, reword } => {
            let (source_branch_name, mut sources) = resolve_commits_on_branch(&branch, repo, ws)?;
            let Some(target) = sources.pop() else {
                return Err(bad_input("Cannot squash empty branch into itself").into());
            };

            ResolvedSquash::Branches {
                target: BranchSquashTarget::Commit { target, reword },
                source_commits: sources,
                source_branches: NonEmpty::new(source_branch_name),
                branches_to_remove: Vec::new(),
            }
        }
    };

    let mut squash_op = match resolved_squash {
        ResolvedSquash::Commits { target, sources } => match target {
            SquashTarget::Commit {
                commit: target,
                reword,
            } => SquashOperation::Commits(SquashCommitsOperation {
                sources,
                target,
                reword,
            }),
            SquashTarget::Uncommitted => {
                SquashOperation::Uncommit(UncommitOperation::Commits { sources })
            }
        },
        ResolvedSquash::Branches {
            target,
            source_commits,
            source_branches,
            branches_to_remove,
        } => match target {
            BranchSquashTarget::Commit { target, reword } => {
                SquashOperation::Branch(SquashBranchOperation {
                    sources: source_commits,
                    target,
                    reword,
                    source_branches,
                    branches_to_remove,
                })
            }
            BranchSquashTarget::Uncommitted => {
                SquashOperation::Uncommit(UncommitOperation::Branches {
                    source_commits,
                    source_branches,
                    branches_to_remove,
                })
            }
        },
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

    ensure_not_touching_merged_upstream(&squash_op, merged)?;

    fix_up_unnecessary_reword_via_editor(&mut squash_op, repo)?;

    Ok(squash_op)
}

/// Reject operations whose sources or target have already landed in the
/// target branch. This covers every entrypoint into the squash machinery:
/// `squash`, `amend`, `uncommit`, and the status TUI.
fn ensure_not_touching_merged_upstream(
    op: &SquashOperation<'_>,
    merged: &MergedUpstream,
) -> CliResult<()> {
    match op {
        SquashOperation::Commits(SquashCommitsOperation {
            sources, target, ..
        }) => {
            merged.ensure_commit_not_merged(target.commit_id)?;
            for source in sources {
                merged.ensure_commit_not_merged(source.commit_id)?;
            }
        }
        SquashOperation::Branch(SquashBranchOperation {
            sources, target, ..
        }) => {
            merged.ensure_commit_not_merged(target.commit_id)?;
            for source in sources {
                merged.ensure_commit_not_merged(source.commit_id)?;
            }
        }
        SquashOperation::UncommittedHunks(AmendUncommittedHunks { target, .. })
        | SquashOperation::Uncommitted { target, .. } => {
            merged.ensure_commit_not_merged(target.commit_id)?;
        }
        SquashOperation::MoveCommittedFiles { target, source, .. } => {
            merged.ensure_commit_not_merged(target.commit_id)?;
            merged.ensure_commit_not_merged(source.commit_id)?;
        }
        SquashOperation::Uncommit(op) => match op {
            UncommitOperation::Commits { sources } => {
                for commit in sources {
                    merged.ensure_commit_not_merged(commit.commit_id)?;
                }
            }
            UncommitOperation::Branches {
                source_commits,
                branches_to_remove,
                source_branches: _,
            } => {
                for commit in source_commits {
                    merged.ensure_commit_not_merged(commit.commit_id)?;
                }
                for branch in branches_to_remove {
                    merged.ensure_branch_not_merged(branch.as_ref())?;
                }
            }
        },
        SquashOperation::UncommitCommittedFiles(UncommitCommittedFilesOperation {
            source, ..
        }) => {
            merged.ensure_commit_not_merged(source.commit_id)?;
        }
    }
    Ok(())
}

fn cannot_uncommit_uncommitted_changes_error() -> CliError {
    bad_input("Cannot uncommit uncommitted changes")
        .hint("When squashing uncommitted changes the --target must be a commit or a branch")
        .into()
}

/// Changes how the operation rewords the target to avoid unnecessarily opening the editor.
///
/// For example if none of the sources have a message then we should always pick the target
/// message, and vice versa.
///
/// If no editor would have been opened then this function does nothing. So we still respect
/// `--use-source-message` and `--use-target-message` flags.
fn fix_up_unnecessary_reword_via_editor(
    op: &mut SquashOperation,
    repo: &gix::Repository,
) -> anyhow::Result<()> {
    fn obvious_final_message<'a, I>(
        commits: I,
        repo: &gix::Repository,
    ) -> anyhow::Result<Option<String>>
    where
        I: IntoIterator<Item = CommitIdRef<'a>>,
    {
        let mut out = None;
        let mut seen = Vec::new();
        for commit in commits {
            if seen.contains(&commit) {
                continue;
            } else {
                seen.push(commit);
            }

            let commit = repo.find_commit(commit.commit_id)?;
            let msg = commit.message_raw()?;
            if msg.is_empty() {
                continue;
            }
            if out.is_some() {
                return Ok(None);
            }
            if let Ok(msg) = msg.to_str() {
                out = Some(msg.to_owned());
            } else {
                // make sure we don't remove non utf-8 messages
                return Ok(None);
            }
        }
        if let Some(out) = out {
            Ok(Some(out))
        } else {
            // none of the commits have messages so just keep an empty message and dont open an
            // editor
            Ok(Some(String::new()))
        }
    }

    if !op.will_open_editor() {
        return Ok(());
    }

    match op {
        SquashOperation::Commits(op) => {
            let commits = op
                .sources
                .iter()
                .map(|c| c.as_ref())
                .chain([op.target.as_ref()]);
            if let Some(msg) = obvious_final_message(commits, repo)? {
                op.reword = HowToRewordTarget::Reword(CommitMessageSource::Provided(msg));
            }
        }
        SquashOperation::Branch(op) => {
            let commits = op
                .sources
                .iter()
                .map(|c| c.as_ref())
                .chain([op.target.as_ref()]);
            if let Some(msg) = obvious_final_message(commits, repo)? {
                op.reword = HowToRewordTarget::Reword(CommitMessageSource::Provided(msg));
            }
        }
        SquashOperation::UncommittedHunks(op) => {
            if let Some(msg) = obvious_final_message([op.target.as_ref()], repo)? {
                op.reword = HowToRewordTargetNoSource::Reword(CommitMessageSource::Provided(msg));
            }
        }
        SquashOperation::Uncommitted { target, reword, .. } => {
            if let Some(msg) = obvious_final_message([target.as_ref()], repo)? {
                *reword = HowToRewordTargetNoSource::Reword(CommitMessageSource::Provided(msg));
            }
        }
        SquashOperation::MoveCommittedFiles { target, reword, .. } => {
            if let Some(msg) = obvious_final_message([target.as_ref()], repo)? {
                *reword = HowToRewordTargetNoSource::Reword(CommitMessageSource::Provided(msg));
            }
        }
        SquashOperation::Uncommit(..) | SquashOperation::UncommitCommittedFiles(..) => {}
    }

    Ok(())
}

#[derive(Debug)]
pub enum ResolvedSquash<'a> {
    Commits {
        target: SquashTarget,
        sources: NonEmpty<CommitId>,
    },
    Branches {
        target: BranchSquashTarget,
        source_commits: Vec<CommitId>,
        /// The branches that we're squashing.
        ///
        /// This is just used to generate the output.
        source_branches: NonEmpty<FullName>,
        /// The branches that should be removed after squashing the commits.
        branches_to_remove: Vec<FullName>,
    },
    UncommittedHunk(AmendUncommittedHunks<'a>),
    Uncommitted {
        target: CommitId,
        reword: HowToRewordTargetNoSource,
    },
    CommittedFiles {
        target: MoveCommittedChangesTarget,
        source: CommitId,
        source_paths: Vec<BString>,
    },
}

#[derive(Debug, Clone)]
pub enum BranchSquashTarget {
    Commit {
        target: CommitId,
        reword: HowToRewordTarget,
    },
    Uncommitted,
}

#[derive(Clone, Debug)]
pub struct AmendUncommittedHunks<'a> {
    pub target: CommitId,
    pub sources: NonEmpty<UncommittedSquashSource<'a>>,
    pub reword: HowToRewordTargetNoSource,
}

impl AmendUncommittedHunks<'_> {
    pub fn into_fully_owned(self) -> AmendUncommittedHunks<'static> {
        AmendUncommittedHunks {
            target: self.target,
            sources: self.sources.map(UncommittedSquashSource::into_owned),
            reword: self.reword,
        }
    }
}

#[derive(Clone, Debug)]
pub enum UncommittedSquashSource<'a> {
    HunkOrFile(Cow<'a, UncommittedHunkOrFile>),
    PathPrefix(Cow<'a, NonEmpty<IdAndHunk>>),
}

impl UncommittedSquashSource<'_> {
    fn into_owned(self) -> UncommittedSquashSource<'static> {
        match self {
            UncommittedSquashSource::HunkOrFile(source) => {
                UncommittedSquashSource::HunkOrFile(Cow::Owned(source.into_owned()))
            }
            UncommittedSquashSource::PathPrefix(source) => {
                UncommittedSquashSource::PathPrefix(Cow::Owned(source.into_owned()))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum SquashTarget {
    Commit {
        commit: CommitId,
        reword: HowToRewordTarget,
    },
    Uncommitted,
}

#[derive(Debug, Clone)]
pub enum MoveCommittedChangesTarget {
    Commit {
        commit: CommitId,
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

pub fn resolve_target(
    target: ResolvedCliIdArgRef<'_>,
    reword: HowToRewordTarget,
    head_info: &RefInfo,
    repo: &gix::Repository,
) -> Result<SquashTarget, ResolveTargetError> {
    match target {
        ResolvedCliIdArgRef::Commit(commit) => Ok(SquashTarget::Commit {
            commit: commit.to_owned(),
            reword,
        }),
        ResolvedCliIdArgRef::Branch(branch_name) => {
            let branch_name = BranchArg(branch_name.to_owned())
                .resolve_local_branch_name()
                .map_err(ResolveTargetError::Other)?;

            for stack in &head_info.stacks {
                for segment in &stack.segments {
                    let Some(ref_info) = &segment.ref_info else {
                        continue;
                    };
                    if ref_info.ref_name == branch_name {
                        let commit = segment
                            .commits
                            .first()
                            .map(|commit| commit.id)
                            .or(ref_info.commit_id);
                        return if let Some(commit) = commit {
                            Ok(SquashTarget::Commit {
                                commit: CommitId::try_from_commit_id(commit, repo)
                                    .map_err(ResolveTargetError::Other)?,
                                reword,
                            })
                        } else {
                            Err(ResolveTargetError::CannotBeEmptyBranch)
                        };
                    }
                }
            }

            Err(ResolveTargetError::NotFound)
        }
        ResolvedCliIdArgRef::Uncommitted => {
            match reword {
                HowToRewordTarget::UseTargetMessage => {
                    return Err(ResolveTargetError::UseTargetMessageUnavailable);
                }
                HowToRewordTarget::UseSourceMessage => {
                    return Err(ResolveTargetError::UseSourceMessageUnavailable);
                }
                HowToRewordTarget::Reword(reword_op) => match reword_op {
                    CommitMessageSource::Empty => {
                        return Err(ResolveTargetError::NoMessageUnavailable);
                    }
                    CommitMessageSource::Provided(_) => {
                        return Err(ResolveTargetError::MessageUnavailable);
                    }
                    CommitMessageSource::Editor { .. } => {}
                },
            }

            Ok(SquashTarget::Uncommitted)
        }
        ResolvedCliIdArgRef::UncommittedHunkOrFile(..)
        | ResolvedCliIdArgRef::CommittedFile { .. }
        | ResolvedCliIdArgRef::PathPrefix { .. }
        | ResolvedCliIdArgRef::Stack { .. } => Err(ResolveTargetError::InvalidTarget),
    }
}

#[derive(Debug)]
pub enum ResolveTargetError {
    CannotBeEmptyBranch,
    NotFound,
    UseTargetMessageUnavailable,
    UseSourceMessageUnavailable,
    NoMessageUnavailable,
    MessageUnavailable,
    InvalidTarget,
    Other(anyhow::Error),
}

pub fn resolve_squash_branch(
    target: SquashTarget,
    branch_sources: NonEmpty<BranchArg>,
    repo: &gix::Repository,
    ws: &Workspace,
) -> CliResult<ResolvedSquash<'static>> {
    let target = match target {
        SquashTarget::Commit { commit, reword } => BranchSquashTarget::Commit {
            target: commit,
            reword,
        },
        SquashTarget::Uncommitted => BranchSquashTarget::Uncommitted,
    };

    let mut source_branches = Vec::<FullName>::new();
    let mut branches_to_remove = Vec::<FullName>::new();
    let mut commits_on_branch_sources = Vec::new();
    for branch_name in branch_sources {
        let (source_branch_name, mut commits_on_branch) =
            resolve_commits_on_branch(&branch_name, repo, ws)?;

        match &target {
            BranchSquashTarget::Commit { target, reword: _ } => {
                let mut target_commit_exists_on_branch = false;
                commits_on_branch.retain(|commit| {
                    if *commit == *target {
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
            }
            BranchSquashTarget::Uncommitted => {
                branches_to_remove.push(source_branch_name.clone());
                commits_on_branch_sources.append(&mut commits_on_branch);
            }
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
    })
}

fn resolve_reword(
    message: Option<Vec<String>>,
    no_message: bool,
    use_target_message: bool,
    use_source_message: bool,
) -> CliResult<HowToRewordTarget> {
    if use_target_message {
        Ok(HowToRewordTarget::UseTargetMessage)
    } else if use_source_message {
        Ok(HowToRewordTarget::UseSourceMessage)
    } else {
        Ok(HowToRewordTarget::Reword(CommitMessageSource::from_args(
            no_message, message,
        )?))
    }
}

#[derive(Debug, Clone)]
pub enum HowToRewordTarget {
    UseTargetMessage,
    UseSourceMessage,
    Reword(CommitMessageSource),
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
        commit: CommitId,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<CommitId> {
        match self {
            Self::UseTargetMessage | Self::UseSourceMessage => Ok(commit),
            Self::Reword(message) => message.execute(commit, tx),
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
pub enum HowToRewordTargetNoSource {
    UseTargetMessage,
    Reword(CommitMessageSource),
}

impl HowToRewordTargetNoSource {
    pub fn will_open_editor(&self) -> bool {
        match self {
            Self::UseTargetMessage => false,
            Self::Reword(op) => op.will_open_editor(),
        }
    }

    fn execute(
        self,
        commit: CommitId,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<CommitId> {
        match self {
            Self::UseTargetMessage => Ok(commit),
            Self::Reword(message) => message.execute(commit, tx),
        }
    }
}

enum Squashable<'a> {
    Commit(CommitId),
    Branch(BranchArg),
    UncommittedChange(UncommittedSquashSource<'a>),
    Uncommitted(&'static str),
    CommittedFile(CommittedFileId),
}

impl<'a> Squashable<'a> {
    fn try_from_resolved_id(id: ResolvedCliIdArgRef<'a>) -> CliResult<Self> {
        let kind = match id {
            ResolvedCliIdArgRef::Commit(commit) => return Ok(Self::Commit(commit.to_owned())),
            ResolvedCliIdArgRef::Branch(branch_name) => {
                return Ok(Self::Branch(BranchArg(branch_name.to_owned())));
            }
            ResolvedCliIdArgRef::UncommittedHunkOrFile(hunk) => {
                return Ok(Self::UncommittedChange(
                    UncommittedSquashSource::HunkOrFile(Cow::Borrowed(hunk)),
                ));
            }
            ResolvedCliIdArgRef::Uncommitted => return Ok(Self::Uncommitted(UNCOMMITTED)),
            ResolvedCliIdArgRef::CommittedFile(file) => {
                return Ok(Self::CommittedFile(file.clone()));
            }
            ResolvedCliIdArgRef::PathPrefix { id: _, hunks } => {
                return Ok(Self::UncommittedChange(
                    UncommittedSquashSource::PathPrefix(Cow::Borrowed(hunks)),
                ));
            }
            ResolvedCliIdArgRef::Stack { .. } => "a stack",
        };
        Err(bad_input(format!(
            "Expected a commit, a branch, or an uncommitted change, got {kind}"
        ))
        .into())
    }
}

enum ClassifiedSquashables<'a> {
    Commits(NonEmpty<CommitId>),
    Branches(NonEmpty<BranchArg>),
    UncommittedChanges(NonEmpty<UncommittedSquashSource<'a>>),
    Uncommitted,
    CommittedFiles(NonEmpty<CommittedFileId>),
}

impl<'a> ClassifiedSquashables<'a> {
    fn try_from_sources(
        commit_sources: Vec<CommitId>,
        branch_sources: Vec<BranchArg>,
        uncommitted_change_sources: Vec<UncommittedSquashSource<'a>>,
        uncommitted_sources: Vec<&'static str>,
        committed_file_sources: Vec<CommittedFileId>,
    ) -> CliResult<Self> {
        let has_commits = !commit_sources.is_empty();
        let has_branches = !branch_sources.is_empty();
        let has_uncommitted_changes = !uncommitted_change_sources.is_empty();
        let has_uncommitted = !uncommitted_sources.is_empty();
        let has_committed_file_sources = !committed_file_sources.is_empty();

        let source_type_count = [
            has_commits,
            has_branches,
            has_uncommitted_changes,
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
        } else if let Some(sources) = NonEmpty::from_vec(uncommitted_change_sources) {
            Ok(Self::UncommittedChanges(sources))
        } else if has_uncommitted {
            Ok(Self::Uncommitted)
        } else if let Some(committed_file_sources) = NonEmpty::from_vec(committed_file_sources) {
            Ok(Self::CommittedFiles(committed_file_sources))
        } else {
            unreachable!("squash resolution always supplies at least one source")
        }
    }
}

pub fn run(
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
            sources,
            reword,
        }) => {
            let context_lines = ctx.settings.context_lines;
            let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&repo, context_lines);
            for source in &sources {
                match source {
                    UncommittedSquashSource::HunkOrFile(source) => {
                        builder.push_changes_from_uncommitted(source)?;
                    }
                    UncommittedSquashSource::PathPrefix(source) => {
                        builder.push_changes_from_path_prefix(source)?;
                    }
                }
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
            let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&repo, context_lines);
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
            let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&repo, context_lines);
            for path in source_paths {
                builder.push_changes_from_committed_file(source.commit_id, path.as_ref())?;
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
        SquashOperation::Uncommit(op) => match op {
            UncommitOperation::Commits { mut sources } => {
                sources = non_empty_dedup_maintain_sort(sources);
                ExecutableSquashOperation::Uncommit(UncommitOperation::Commits { sources })
            }
            UncommitOperation::Branches {
                mut source_commits,
                mut source_branches,
                mut branches_to_remove,
            } => {
                source_commits.sort();
                source_commits.dedup();

                branches_to_remove.sort();
                branches_to_remove.dedup();

                source_branches = non_empty_dedup_maintain_sort(source_branches);

                ExecutableSquashOperation::Uncommit(UncommitOperation::Branches {
                    source_commits,
                    source_branches,
                    branches_to_remove,
                })
            }
        },
        SquashOperation::UncommitCommittedFiles(UncommitCommittedFilesOperation {
            source,
            source_paths,
        }) => {
            let context_lines = ctx.settings.context_lines;
            let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&repo, context_lines);
            for path in source_paths {
                builder.push_changes_from_committed_file(source.commit_id, path.as_ref())?;
            }
            let changes = builder.into_diff_specs();

            ExecutableSquashOperation::UncommitHunks { source, changes }
        }
    };

    match executable_op {
        ExecutableSquashOperation::TransactionCompatible(op) => {
            let snapshot_details = SnapshotDetails::new(OperationKind::SquashCommit);
            let (new_commit, _ws) = but_transaction::with_transaction_with_perm(
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

                    Ok(but_transaction::Commit(new_commit))
                },
            )
            .map_err(|err| {
                // Only the amend path can reject changes; other errors pass
                // through `explain_after_rollback` untouched.
                let TransactionCompatibleOperation::UncommittedHunks(
                    AmendUncommittedDiffSpecsOperation { target, .. },
                ) = &op
                else {
                    return err;
                };
                let target = rejection::Target::Commit(target.clone());
                rejection::explain_after_rollback(ctx, perm, "amend", target, err)
            })?;

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
            if op.has_commit_sources() {
                let but_api::commit::types::UncommitResult {
                    workspace,
                    uncommitted_ids: _,
                } = but_api::commit::uncommit::commit_uncommit_only_with_perm(
                    ctx,
                    op.commit_sources().collect(),
                    None,
                    DryRun::Yes,
                    perm,
                )?;

                anyhow::ensure!(
                    !workspace.is_conflicted(),
                    "Cannot uncommit commits that would result in merge conflicts"
                );
            }

            let snapshot_details = SnapshotDetails::new(OperationKind::UndoCommit);
            let _ws = but_transaction::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
                DryRun::No,
                |mut tx| {
                    match &op {
                        UncommitOperation::Commits { sources } => {
                            if !sources.is_empty() {
                                tx.uncommit_commits(sources.iter().map(|c| c.commit_id))?;
                            }
                        }
                        UncommitOperation::Branches {
                            source_commits,
                            branches_to_remove,
                            source_branches: _,
                        } => {
                            if !source_commits.is_empty() {
                                tx.uncommit_commits(source_commits.iter().map(|c| c.commit_id))?;
                            }
                            for branch_name in branches_to_remove {
                                tx.remove_reference(branch_name.as_ref())?;
                            }
                        }
                    }

                    Ok(())
                },
            )?;

            match op {
                UncommitOperation::Commits { sources } => Ok(SquashOutcome::UncommitCommit {
                    sources: sources.into_iter().collect(),
                }),
                UncommitOperation::Branches {
                    source_branches,
                    source_commits: _,
                    branches_to_remove: _,
                } => Ok(SquashOutcome::UncommitBranch {
                    branch_names: source_branches,
                }),
            }
        }
        ExecutableSquashOperation::UncommitHunks { source, changes } => {
            {
                let but_api::commit::types::MoveChangesResult { workspace } =
                    but_api::commit::uncommit::commit_uncommit_changes_only_with_perm(
                        ctx,
                        source.commit_id,
                        changes.clone(),
                        None,
                        DryRun::Yes,
                        perm,
                    )?;

                anyhow::ensure!(
                    !workspace.is_conflicted(),
                    "Cannot uncommit hunks that would result in merge conflicts"
                );
            }

            let but_api::commit::types::MoveChangesResult { workspace: _ } =
                but_api::commit::uncommit::commit_uncommit_changes_with_perm(
                    ctx,
                    source.commit_id,
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
pub enum SquashOperation<'a> {
    Commits(SquashCommitsOperation),
    Branch(SquashBranchOperation),
    UncommittedHunks(AmendUncommittedHunks<'a>),
    Uncommitted {
        target: CommitId,
        reword: HowToRewordTargetNoSource,
    },
    MoveCommittedFiles {
        target: CommitId,
        source: CommitId,
        source_paths: Vec<BString>,
        reword: HowToRewordTargetNoSource,
    },
    Uncommit(UncommitOperation),
    UncommitCommittedFiles(UncommitCommittedFilesOperation),
}

impl SquashOperation<'_> {
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

    pub fn into_fully_owned(self) -> SquashOperation<'static> {
        match self {
            SquashOperation::Commits(inner) => SquashOperation::Commits(inner),
            SquashOperation::Branch(inner) => SquashOperation::Branch(inner),
            SquashOperation::UncommittedHunks(inner) => {
                SquashOperation::UncommittedHunks(inner.into_fully_owned())
            }
            SquashOperation::Uncommitted { target, reword } => {
                SquashOperation::Uncommitted { target, reword }
            }
            SquashOperation::MoveCommittedFiles {
                target,
                source,
                source_paths,
                reword,
            } => SquashOperation::MoveCommittedFiles {
                target,
                source,
                source_paths,
                reword,
            },
            SquashOperation::Uncommit(inner) => SquashOperation::Uncommit(inner),
            SquashOperation::UncommitCommittedFiles(inner) => {
                SquashOperation::UncommitCommittedFiles(inner)
            }
        }
    }
}

#[derive(Clone)]
enum ExecutableSquashOperation {
    TransactionCompatible(TransactionCompatibleOperation),
    // Uncommitting requires special transaction handling
    Uncommit(UncommitOperation),
    UncommitHunks {
        source: CommitId,
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
pub struct SquashCommitsOperation {
    pub sources: NonEmpty<CommitId>,
    pub target: CommitId,
    pub reword: HowToRewordTarget,
}

impl SquashCommitsOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<CommitId> {
        let Self {
            sources,
            target,
            reword,
        } = self;
        let new_commit = tx.squash_commits(
            sources.iter().map(|c| c.commit_id),
            target.commit_id,
            reword.how_to_combine_messages(),
        )?;
        reword.execute(new_commit.into(), tx)
    }
}

#[derive(Clone)]
pub struct SquashBranchOperation {
    pub sources: Vec<CommitId>,
    pub target: CommitId,
    pub reword: HowToRewordTarget,
    pub source_branches: NonEmpty<FullName>,
    pub branches_to_remove: Vec<FullName>,
}

impl SquashBranchOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<CommitId> {
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

        let new_commit = tx.squash_commits(
            sources.iter().map(|c| c.commit_id),
            target.commit_id,
            reword.how_to_combine_messages(),
        )?;
        reword.execute(new_commit.into(), tx)
    }
}

#[derive(Clone)]
struct AmendUncommittedDiffSpecsOperation {
    target: CommitId,
    changes: Vec<DiffSpec>,
    reword: HowToRewordTargetNoSource,
}

impl AmendUncommittedDiffSpecsOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<CommitId> {
        let Self {
            target,
            changes,
            reword,
        } = self;

        let IntermediateCommitCreateResult {
            new_commit,
            rejected_specs,
        } = tx.amend_commit(target.commit_id, changes, ChangeSource::Head)?;

        if !rejected_specs.is_empty() {
            return Err(rejection::RejectedChanges(rejected_specs).into());
        }

        let new_commit =
            new_commit.context("BUG: rejected_specs is empty yet nothing was committed")?;

        reword.execute(new_commit.into(), tx)
    }
}

#[derive(Clone)]
struct MoveCommittedFilesOperation {
    target: CommitId,
    source: CommitId,
    changes: Vec<but_core::DiffSpec>,
    reword: HowToRewordTargetNoSource,
}

impl MoveCommittedFilesOperation {
    fn execute(self, tx: &mut Transaction<'_, '_, impl RefMetadata>) -> anyhow::Result<CommitId> {
        let Self {
            target,
            source,
            changes,
            reword,
        } = self;

        let new_commit =
            tx.move_committed_changes_between(source.commit_id, target.commit_id, changes)?;
        reword.execute(new_commit.into(), tx)
    }
}

#[derive(Clone)]
pub enum UncommitOperation {
    Commits {
        sources: NonEmpty<CommitId>,
    },
    Branches {
        source_commits: Vec<CommitId>,
        source_branches: NonEmpty<FullName>,
        branches_to_remove: Vec<FullName>,
    },
}

impl UncommitOperation {
    fn has_commit_sources(&self) -> bool {
        match self {
            UncommitOperation::Commits { sources } => !sources.is_empty(),
            UncommitOperation::Branches { source_commits, .. } => !source_commits.is_empty(),
        }
    }

    fn commit_sources(&self) -> impl Iterator<Item = ObjectId> {
        match self {
            UncommitOperation::Commits { sources } => {
                Either::Right(sources.iter().map(|c| c.commit_id))
            }
            UncommitOperation::Branches { source_commits, .. } => {
                Either::Left(source_commits.iter().map(|c| c.commit_id))
            }
        }
    }
}

#[derive(Clone)]
pub struct UncommitCommittedFilesOperation {
    pub source: CommitId,
    pub source_paths: Vec<BString>,
}

fn resolve_commits_on_branch(
    branch: &BranchArg,
    repo: &gix::Repository,
    ws: &Workspace,
) -> CliResult<(FullName, Vec<CommitId>)> {
    let branch_name = branch.resolve_local_branch_name()?;
    let commits_in_segment = resolve_commits_on_branch_by_ref(branch_name.as_ref(), repo, ws)?;
    Ok((branch_name, commits_in_segment))
}

pub fn resolve_commits_on_branch_by_ref(
    branch: &FullNameRef,
    repo: &gix::Repository,
    ws: &Workspace,
) -> anyhow::Result<Vec<CommitId>> {
    let (_, segment) = ws.try_find_segment_and_stack_by_refname(branch)?;
    let commits_in_segment = segment
        .commits
        .iter()
        .map(|commit| commit.id)
        .map(|id| CommitId::try_from_commit_id(id, repo))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(commits_in_segment)
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
