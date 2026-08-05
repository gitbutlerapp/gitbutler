//! Implementation of the `but discard` command.

use std::collections::BTreeMap;

use anyhow::{Context as _, bail};
use bstr::{BString, ByteSlice as _};
use but_api::json::{ChangeIdString, HexHash};
use but_core::{DiffSpec, DryRun, RefMetadata, sync::RepoExclusive};
use but_ctx::Context;
use but_transaction::Commit;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{ObjectId, refs::FullName};
use itertools::Itertools;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{BranchArg, Purpose, ResolvedCliIdArg},
        discard::Platform,
    },
    bad_input,
    id::{CommitId, CommittedFileId, IdAndHunk, UncommittedHunkOrFile},
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, CommitIdJson, IntermediateChannel, WriteWithUtils,
        diff_specs::DiffSpecBuilder,
    },
};

#[derive(Debug)]
pub enum DiscardOperation {
    Branches(NonEmpty<FullName>),
    Commits(NonEmpty<CommitId>),
    CommittedFiles {
        source: CommitId,
        paths: NonEmpty<BString>,
    },
    Uncommitted(UncommittedSelection),
}

#[derive(Debug)]
enum ClassifiedDiscardables {
    Commits(NonEmpty<CommitId>),
    Branches(NonEmpty<BranchArg>),
    UncommittedChanges(NonEmpty<UncommittedDiscardSource>),
    Uncommitted,
    CommittedFiles(NonEmpty<CommittedFileId>),
}

impl ClassifiedDiscardables {
    fn try_from_sources(
        commit_sources: Vec<CommitId>,
        branch_sources: Vec<BranchArg>,
        uncommitted_change_sources: Vec<UncommittedDiscardSource>,
        uncommitted_sources: Vec<()>,
        committed_file_sources: Vec<CommittedFileId>,
    ) -> CliResult<Self> {
        let has_commits = !commit_sources.is_empty();
        let has_branches = !branch_sources.is_empty();
        let has_uncommitted_changes = !uncommitted_change_sources.is_empty();
        let has_uncommitted = !uncommitted_sources.is_empty();
        let has_committed_files = !committed_file_sources.is_empty();

        let source_type_count = [
            has_commits,
            has_branches,
            has_uncommitted_changes,
            has_uncommitted,
            has_committed_files,
        ]
        .into_iter()
        .filter(|has_source| *has_source)
        .count();

        if source_type_count > 1 {
            return Err(bad_input("Cannot mix different types of sources")
                .arg_name("<CHANGES>")
                .hint(
                    "Discard branches, commits, committed files, or uncommitted changes separately",
                )
                .into());
        }

        if let Some(commits) = NonEmpty::from_vec(commit_sources) {
            Ok(Self::Commits(commits))
        } else if let Some(branches) = NonEmpty::from_vec(branch_sources) {
            Ok(Self::Branches(branches))
        } else if let Some(changes) = NonEmpty::from_vec(uncommitted_change_sources) {
            Ok(Self::UncommittedChanges(changes))
        } else if has_uncommitted {
            Ok(Self::Uncommitted)
        } else if let Some(files) = NonEmpty::from_vec(committed_file_sources) {
            Ok(Self::CommittedFiles(files))
        } else {
            Ok(Self::Uncommitted)
        }
    }
}

#[derive(Debug)]
pub enum UncommittedSelection {
    All,
    Changes(Box<NonEmpty<UncommittedDiscardSource>>),
}

impl UncommittedSelection {
    pub fn changes(changes: NonEmpty<UncommittedHunkOrFile>) -> Self {
        Self::Changes(Box::new(changes.map(UncommittedDiscardSource::HunkOrFile)))
    }
}

#[derive(Debug)]
pub enum UncommittedDiscardSource {
    HunkOrFile(UncommittedHunkOrFile),
    PathPrefix(NonEmpty<IdAndHunk>),
}

#[must_use]
pub enum DiscardOutcome {
    Branches(NonEmpty<FullName>),
    Commits {
        commits: NonEmpty<CommitId>,
        /// Rewritten surviving commits, used by callers to retain their selection.
        replaced_commits: BTreeMap<ObjectId, ObjectId>,
    },
    CommittedFiles {
        source: CommitId,
        paths: NonEmpty<BString>,
        new_commit: CommitId,
    },
    Uncommitted {
        paths: NonEmpty<BString>,
    },
}

impl CliOutputHuman for DiscardOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            DiscardOutcome::Branches(branches) => {
                if branches.len() == 1 {
                    writeln!(out, "Discarded branch {}", theme::Branch(&branches.head))?;
                } else {
                    let branches = branches.iter().map(theme::Branch).join(", ");
                    writeln!(out, "Discarded branches {branches}")?;
                }
            }
            DiscardOutcome::Commits {
                commits,
                replaced_commits: _,
            } => {
                if commits.len() == 1 {
                    writeln!(out, "Discarded commit {}", theme::Commit(commits.head))?;
                } else {
                    let commits = commits.iter().map(theme::Commit).join(", ");
                    writeln!(out, "Discarded commits {commits}")?;
                }
            }
            DiscardOutcome::CommittedFiles {
                source,
                paths,
                new_commit,
            } => {
                let paths = paths.iter().map(|path| path.as_bstr()).join(", ");
                writeln!(
                    out,
                    "Discarded {paths} from {} to create {}",
                    theme::Commit(source),
                    theme::Commit(new_commit)
                )?;
            }
            DiscardOutcome::Uncommitted { paths } => {
                let paths = paths.iter().map(|path| path.as_bstr()).join(", ");
                writeln!(out, "Discarded uncommitted changes from {paths}")?;
            }
        }

        Ok(())
    }
}

impl CliOutput for DiscardOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(
            tag = "type",
            rename_all = "camelCase",
            rename_all_fields = "camelCase"
        )]
        enum Output {
            Branches {
                branches: Vec<String>,
            },
            Commits {
                commits: Vec<CommitIdJson>,
            },
            CommittedFiles {
                source_commit_id: HexHash,
                #[serde(skip_serializing_if = "Option::is_none")]
                source_change_id: Option<ChangeIdString>,
                paths: Vec<String>,
                new_commit_id: HexHash,
                #[serde(skip_serializing_if = "Option::is_none")]
                new_change_id: Option<ChangeIdString>,
            },
            UncommittedChanges {
                paths: Vec<String>,
            },
        }

        match self {
            DiscardOutcome::Branches(branches) => Output::Branches {
                branches: branches
                    .into_iter()
                    .map(|branch| branch.shorten().to_string())
                    .collect(),
            },
            DiscardOutcome::Commits {
                commits,
                replaced_commits: _,
            } => Output::Commits {
                commits: commits.into_iter().map(Into::into).collect(),
            },
            DiscardOutcome::CommittedFiles {
                source,
                paths,
                new_commit,
            } => Output::CommittedFiles {
                source_commit_id: source.commit_id.into(),
                source_change_id: source.change_id.map(Into::into),
                new_commit_id: new_commit.commit_id.into(),
                new_change_id: new_commit.change_id.map(Into::into),
                paths: paths
                    .into_iter()
                    .map(|path| path.to_str_lossy().into_owned())
                    .collect(),
            },
            DiscardOutcome::Uncommitted { paths } => Output::UncommittedChanges {
                paths: paths
                    .into_iter()
                    .map(|path| path.to_str_lossy().into_owned())
                    .collect(),
            },
        }
    }
}

pub fn discard(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<DiscardOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let operation = {
        let repo = ctx.repo.get()?;
        resolve(&repo, &id_map, args)?
    };

    Ok(run(ctx, &mut meta, guard.write_permission(), operation)?)
}

fn resolve(repo: &gix::Repository, id_map: &IdMap, args: Platform) -> CliResult<DiscardOperation> {
    let Platform { changes } = args;

    let mut branch_sources = Vec::new();
    let mut commit_sources = Vec::new();
    let mut committed_file_sources = Vec::new();
    let mut uncommitted_change_sources = Vec::new();
    let mut uncommitted_sources = Vec::new();

    for change in changes {
        let value = change.to_string();
        match change.resolve_in_workspace(repo, id_map, Purpose::Source, None)? {
            ResolvedCliIdArg::Branch(branch) => branch_sources.push(branch),
            ResolvedCliIdArg::Commit(commit) => commit_sources.push(commit),
            ResolvedCliIdArg::CommittedFile(committed_file) => {
                committed_file_sources.push(committed_file)
            }
            ResolvedCliIdArg::UncommittedHunkOrFile(change) => {
                uncommitted_change_sources.push(UncommittedDiscardSource::HunkOrFile(*change))
            }
            ResolvedCliIdArg::Uncommitted => uncommitted_sources.push(()),
            ResolvedCliIdArg::PathPrefix { id: _, hunks } => {
                uncommitted_change_sources.push(UncommittedDiscardSource::PathPrefix(hunks))
            }
            ResolvedCliIdArg::Stack { .. } => {
                return Err(bad_input("Stacks cannot be discarded")
                    .arg_name("<CHANGES>")
                    .arg_value(value)
                    .hint("Use branch CLI IDs instead")
                    .into());
            }
        }
    }

    let classified = ClassifiedDiscardables::try_from_sources(
        commit_sources,
        branch_sources,
        uncommitted_change_sources,
        uncommitted_sources,
        committed_file_sources,
    )?;

    match classified {
        ClassifiedDiscardables::Branches(branches) => {
            let branches = branches
                .into_iter()
                .map(|branch| branch.resolve_local_branch_name())
                .collect::<anyhow::Result<Vec<_>>>()?
                .into_iter()
                .unique()
                .collect();
            let branches = NonEmpty::from_vec(branches)
                .expect("classified branches are guaranteed to be non-empty");
            Ok(DiscardOperation::Branches(branches))
        }
        ClassifiedDiscardables::Commits(commits) => Ok(DiscardOperation::Commits(commits)),
        ClassifiedDiscardables::CommittedFiles(committed_files) => {
            let NonEmpty { head, tail } = committed_files;
            let CommittedFileId {
                commit_id,
                path,
                change_id,
            } = head;
            let source = CommitId {
                commit_id,
                change_id,
            };
            let mut paths = vec![path];
            for CommittedFileId {
                commit_id,
                path,
                change_id: _,
            } in tail
            {
                if commit_id != source.commit_id {
                    return Err(
                        bad_input("All committed files must come from the same commit")
                            .arg_name("<CHANGES>")
                            .hint("Discard committed files from each commit separately")
                            .into(),
                    );
                }
                paths.push(path);
            }
            let paths = paths.into_iter().unique().collect();
            let paths = NonEmpty::from_vec(paths)
                .expect("committed files being non-empty means paths are non-empty");
            Ok(DiscardOperation::CommittedFiles { source, paths })
        }
        ClassifiedDiscardables::Uncommitted => {
            Ok(DiscardOperation::Uncommitted(UncommittedSelection::All))
        }
        ClassifiedDiscardables::UncommittedChanges(changes) => Ok(DiscardOperation::Uncommitted(
            UncommittedSelection::Changes(Box::new(changes)),
        )),
    }
}

pub fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    operation: DiscardOperation,
) -> anyhow::Result<DiscardOutcome> {
    let executable = match operation {
        DiscardOperation::Branches(branches) => {
            let commits = {
                let (repo, workspace, _db) =
                    ctx.workspace_and_db_with_perm(perm.read_permission())?;
                let mut commits = Vec::new();
                for branch in &branches {
                    let (_stack, segment) = workspace
                        .try_find_segment_and_stack_by_refname(branch.as_ref())
                        .with_context(|| {
                            format!(
                                "Could not find branch {} in the workspace",
                                branch.shorten()
                            )
                        })?;
                    for commit in &segment.commits {
                        let commit = CommitId::try_from_commit_id(commit.id, &repo)?;
                        commits.push(commit);
                    }
                }
                commits
            };
            ExecutableDiscardOperation::Branches { branches, commits }
        }
        DiscardOperation::Commits(commits) => ExecutableDiscardOperation::Commits(commits),
        DiscardOperation::CommittedFiles { source, paths } => {
            let changes = {
                let context_lines = ctx.settings.context_lines;
                let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
                let mut builder = DiffSpecBuilder::new(&repo, context_lines);
                for path in &paths {
                    builder.push_changes_from_committed_file(source.commit_id, path.as_bstr())?;
                }
                builder.into_diff_specs()
            };
            anyhow::ensure!(!changes.is_empty(), "No committed changes to discard");
            ExecutableDiscardOperation::CommittedFiles {
                source,
                paths,
                changes,
            }
        }
        DiscardOperation::Uncommitted(selection) => {
            let changes = {
                let context_lines = ctx.settings.context_lines;
                let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
                let mut builder = DiffSpecBuilder::new(&repo, context_lines);
                match selection {
                    UncommittedSelection::All => builder.push_changes_from_uncommitted_area()?,
                    UncommittedSelection::Changes(changes) => {
                        for change in *changes {
                            match change {
                                UncommittedDiscardSource::HunkOrFile(change) => {
                                    builder.push_changes_from_uncommitted(&change)?;
                                }
                                UncommittedDiscardSource::PathPrefix(hunks) => {
                                    builder.push_changes_from_path_prefix(&hunks)?;
                                }
                            }
                        }
                    }
                }
                builder.reconcile_worktree_diff_specs()?;
                builder.into_diff_specs()
            };
            anyhow::ensure!(!changes.is_empty(), "No uncommitted changes to discard");
            let paths = paths_from_changes(&changes);
            ExecutableDiscardOperation::Uncommitted { paths, changes }
        }
    };

    let (mut outcome, workspace) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        SnapshotDetails::new(OperationKind::Discard),
        DryRun::No,
        |mut tx| {
            let outcome = match executable {
                ExecutableDiscardOperation::Branches { branches, commits } => {
                    for branch in &branches {
                        tx.remove_reference(branch.as_ref())?;
                    }
                    if !commits.is_empty() {
                        tx.discard_commits(commits.iter().map(|c| c.commit_id))?;
                    }
                    DiscardOutcome::Branches(branches)
                }
                ExecutableDiscardOperation::Commits(commits) => {
                    tx.discard_commits(commits.iter().map(|c| c.commit_id))?;
                    DiscardOutcome::Commits {
                        commits,
                        replaced_commits: BTreeMap::new(),
                    }
                }
                ExecutableDiscardOperation::CommittedFiles {
                    source,
                    paths,
                    changes,
                } => {
                    let new_commit = tx.discard_changes_from_commit(source.commit_id, changes)?;
                    DiscardOutcome::CommittedFiles {
                        source,
                        paths,
                        new_commit: new_commit.into(),
                    }
                }
                ExecutableDiscardOperation::Uncommitted { paths, changes } => {
                    let refused = but_workspace::discard_workspace_changes(
                        tx.repo(),
                        changes,
                        tx.context_lines(),
                    )?;
                    if !refused.is_empty() {
                        let refused_paths = refused
                            .iter()
                            .map(|change| change.path.as_bstr())
                            .join(", ");
                        bail!("Could not discard all selected changes: {refused_paths}");
                    }
                    DiscardOutcome::Uncommitted { paths }
                }
            };

            Ok(Commit(outcome))
        },
    )?;

    if let DiscardOutcome::Commits {
        replaced_commits, ..
    } = &mut outcome
    {
        *replaced_commits = workspace.replaced_commits;
    }

    Ok(outcome)
}

#[derive(Debug)]
enum ExecutableDiscardOperation {
    Branches {
        branches: NonEmpty<FullName>,
        commits: Vec<CommitId>,
    },
    Commits(NonEmpty<CommitId>),
    CommittedFiles {
        source: CommitId,
        paths: NonEmpty<BString>,
        changes: Vec<DiffSpec>,
    },
    Uncommitted {
        paths: NonEmpty<BString>,
        changes: Vec<DiffSpec>,
    },
}

fn paths_from_changes(changes: &[DiffSpec]) -> NonEmpty<BString> {
    let paths = changes
        .iter()
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    NonEmpty::from_vec(paths).expect("changes being non-empty means paths are non-empty")
}
