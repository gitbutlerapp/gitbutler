use anyhow::Context as _;
use but_api::json::{ChangeIdString, HexHash};
use but_core::{
    DiffSpec, DryRun, RefMetadata,
    ref_metadata::StackId,
    sync::{RepoExclusive, RepoExclusiveGuard},
};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::{IntermediateCommitCreateResult, Transaction};
use but_workspace::{RefInfo, branch::create_reference::Anchor, commit::ChangeSource};
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::refs::FullName;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliError, CliId, CliResult, CliResultExt, IdMap,
    args::{
        atoms::{BranchArg, BranchOrCommit, CliIdArg, Purpose},
        commit::Platform,
    },
    bad_input,
    command::legacy::{
        reword2::CommitMessageSource,
        status::{Selectable, TuiOutcome, TuiRunOptions, tui_with_options},
    },
    error::BadInput,
    id::{CommitId, UncommittedHunkOrFile},
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        diff_specs::DiffSpecBuilder, merged_upstream::MergedUpstream, rejection, targeting::Side,
    },
};

#[must_use]
pub struct CommitOutcome {
    pub new_commit: CommitId,
    pub branch_name: Option<BranchNameTarget>,
}

/// `--json` should only include newly created things. So if the branch already existed it
/// wont be included in the JSON output.
#[derive(Debug)]
pub enum BranchNameTarget {
    Existing(FullName),
    New(FullName),
}

impl CliOutputHuman for CommitOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &Theme,
    ) -> anyhow::Result<()> {
        let Self {
            new_commit,
            branch_name,
        } = self;

        match branch_name {
            Some(BranchNameTarget::New(branch_name)) => writeln!(
                out,
                "Created commit {} on new branch {}",
                theme::Commit(new_commit),
                theme::Branch(branch_name),
            )?,
            Some(BranchNameTarget::Existing(branch_name)) => writeln!(
                out,
                "Created commit {} on branch {}",
                theme::Commit(new_commit),
                theme::Branch(branch_name),
            )?,
            None => writeln!(out, "Created commit {}", theme::Commit(new_commit))?,
        }

        Ok(())
    }
}

impl CliOutput for CommitOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            commit_id: HexHash,
            #[serde(skip_serializing_if = "Option::is_none")]
            change_id: Option<ChangeIdString>,
            #[serde(skip_serializing_if = "Option::is_none")]
            branch: Option<String>,
        }

        let Self {
            new_commit,
            branch_name,
        } = self;

        let branch_name = match branch_name {
            Some(BranchNameTarget::New(branch_name)) => Some(branch_name.shorten().to_string()),
            _ => None,
        };

        Output {
            commit_id: new_commit.commit_id.into(),
            change_id: new_commit.change_id.map(Into::into),
            branch: branch_name,
        }
    }
}

pub fn commit(
    ctx: &mut Context,
    mut out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<CommitOutcome> {
    let guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let (mut guard, commit_op, commit_selection, reword_op) = {
        let head_info = but_api::legacy::workspace::head_info(ctx)?;
        resolve(guard, ctx, args, &mut out, &head_info, &id_map)?
    };
    Ok(run(
        ctx,
        &mut meta,
        guard.write_permission(),
        commit_op,
        commit_selection,
        reword_op,
    )?)
}

fn resolve(
    guard: RepoExclusiveGuard,
    ctx: &mut Context,
    args: Platform,
    out: &mut IntermediateChannel<'_>,
    head_info: &RefInfo,
    id_map: &IdMap,
) -> CliResult<(
    RepoExclusiveGuard,
    CommitOperation,
    CommitSelection,
    CommitMessageSource,
)> {
    let Platform {
        no_message,
        message,
        branch,
        empty,
        above,
        below,
        interactive,
        changes,
        allow_merged,
    } = args;

    let merged = MergedUpstream::new(&*ctx.repo.get()?, head_info, allow_merged);

    let target_ish = CommitOperationTargetIsh::resolve(branch, above, below)?;

    let (guard, commit_selection) = if !changes.is_empty() {
        let changes = changes
            .into_iter()
            .map(|change| {
                let repo = ctx.repo.get()?;
                match change.try_resolve_uncommitted(&repo, id_map)? {
                    Some(resolved) => Ok(resolved),
                    None => Err(unresolved_change_error(&change, &repo, id_map)),
                }
            })
            .collect::<CliResult<Vec<Vec<UncommittedHunkOrFile>>>>()?;
        let changes = changes.into_iter().flatten().collect();
        let Some(changes) = NonEmpty::from_vec(changes) else {
            return Err(bad_input("No changes to commit")
                .hint("Run `but status` to show applicable targets")
                .into());
        };
        (guard, CommitSelection::Changes(Box::new(changes)))
    } else if interactive {
        let Some(mut inout) = out.prepare_for_terminal_input() else {
            return Err(bad_input("Terminal doesn't support interactivity").into());
        };
        let (guard, outcome) =
            tui_with_options(ctx, guard, &mut inout, TuiRunOptions::PickChanges)?;
        let ids = match outcome {
            TuiOutcome::Selection(ids) => ids,
            TuiOutcome::None => {
                return Err(bad_input("No changes to commit")
                    .hint("Pick changes by pressing space. Confirm with enter.")
                    .into());
            }
        };
        let changes = ids
            .into_iter()
            .map(|change| {
                match change {
                    Selectable::UncommittedHunkOrFile(id) => Ok(id),
                    _ => {
                        Err(anyhow::anyhow!("BUG: tui should only return uncommitted changes in PickChanges mode but got {change:?}"))
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let Some(changes) = NonEmpty::from_vec(changes) else {
            return Err(bad_input("No changes to commit")
                .hint("Pick changes by pressing space. Confirm with enter.")
                .into());
        };
        (guard, CommitSelection::Changes(Box::new(changes)))
    } else if empty {
        (guard, CommitSelection::Nothing)
    } else {
        (guard, CommitSelection::AllChanges)
    };

    let commit_op = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        route_commit_operation(&repo, &ws, head_info, out, id_map, target_ish, &merged).map_err(
            |err| match err {
                RouteCommitOperationError::NoStackToCommitTo => {
                    bad_input("Found no stack that could be committed to").into()
                }
                RouteCommitOperationError::UnclearTargetCantPrompt => {
                    bad_input("Unclear where to commit. Found more than one stack")
                        .hint("You can specify where to commit with `--branch [<BRANCH>]`")
                        .into()
                }
                RouteCommitOperationError::Other(cli_error) => cli_error,
            },
        )?
    };

    let reword_op = CommitMessageSource::from_args(no_message, message)?;

    Ok((guard, commit_op, commit_selection, reword_op))
}

/// The retired syntax put the target branch in positional position
/// (`but commit <branch> -m "message"`), which the modern grammar reads as a
/// change. When a change fails to resolve but names an applied branch,
/// suggest `-b` targeting instead of the generic missing-target hint.
fn unresolved_change_error(change: &CliIdArg, repo: &gix::Repository, id_map: &IdMap) -> CliError {
    let names_branch = change
        .parse(repo, id_map)
        .ok()
        .into_iter()
        .flatten()
        .any(|id| matches!(id, CliId::Branch(..)));
    let err = bad_input(format!("Could not find uncommitted change: '{change}'"));
    if names_branch {
        err.hint(format!(
            "'{change}' is a branch. To commit onto it, run `but commit -b {change} -m \"message\" [<change>...]`"
        ))
        .into()
    } else {
        err.hint(CliIdArg::TARGET_MISSING_HINT).into()
    }
}

pub fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    commit_op: CommitOperation,
    commit_selection: CommitSelection,
    reword_op: CommitMessageSource,
) -> anyhow::Result<CommitOutcome> {
    let changes = {
        let context_lines = ctx.settings.context_lines;
        let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
        let mut builder = DiffSpecBuilder::new(&repo, context_lines);

        match commit_selection {
            CommitSelection::AllChanges => {
                builder.push_changes_from_uncommitted_area()?;
            }
            CommitSelection::Changes(changes) => {
                for change in *changes {
                    builder.push_changes_from_uncommitted(&change)?;
                }

                builder.reconcile_worktree_diff_specs()?;
            }
            CommitSelection::Nothing => {}
        }

        builder.into_diff_specs()
    };
    let rejection_target = commit_op.rejection_target();
    let snapshot_details = SnapshotDetails::new(OperationKind::CreateCommit);
    let ((new_commit, branch_name), _ws) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let (
                IntermediateCommitCreateResult {
                    new_commit,
                    rejected_specs,
                },
                branch_name,
            ) = commit_op.execute(&mut tx, changes)?;

            if !rejected_specs.is_empty() {
                return Err(rejection::RejectedChanges(rejected_specs).into());
            }

            let new_commit =
                new_commit.context("BUG: rejected_specs is empty yet nothing was committed")?;

            let reworded_commit = reword_op.execute(new_commit.into(), &mut tx)?;

            Ok(but_transaction::Commit((reworded_commit, branch_name)))
        },
    )
    .map_err(|err| rejection::explain_after_rollback(ctx, perm, "commit", rejection_target, err))?;

    Ok(CommitOutcome {
        new_commit,
        branch_name,
    })
}

/// Targeting modes for committing.
pub enum CommitOperationTargetIsh {
    /// Target the branch if it exists, or create it at the newest base if it does not.
    Branch(CliIdArg),
    /// Target newest base with a new canned branch name.
    UnstackedCannedBranch,
    /// Targets above the [`CliIdArg`], which must denote either a commit or a branch.
    Above(CliIdArg),
    /// Targets below the [`CliIdArg`], which must denote either a commit or a branch. For commits,
    /// this is directly below. For branches, this is below the segment.
    Below(CliIdArg),
    /// The default target, makes a sensible choice about where to put the commit, creating a branch
    /// if necessary. This should be used if there is no input from the user about where to put the
    /// commit.
    Default,
}

impl CommitOperationTargetIsh {
    pub fn resolve(
        branch: Option<Option<CliIdArg>>,
        above: Option<CliIdArg>,
        below: Option<CliIdArg>,
    ) -> CliResult<Self> {
        Ok(match (branch, above, below) {
            (Some(Some(branch)), None, None) => CommitOperationTargetIsh::Branch(branch),
            (Some(None), None, None) => CommitOperationTargetIsh::UnstackedCannedBranch,
            (None, Some(cli_id), None) => CommitOperationTargetIsh::Above(cli_id),
            (None, None, Some(cli_id)) => CommitOperationTargetIsh::Below(cli_id),
            (None, None, None) => CommitOperationTargetIsh::Default,
            _ => {
                return Err(anyhow::anyhow!(
                    "BUG: Should not be able to supply more than one of above, below or branch"
                )
                .into());
            }
        })
    }
}

pub fn route_commit_operation(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    head_info: &RefInfo,
    out: &mut IntermediateChannel<'_>,
    id_map: &IdMap,
    target: CommitOperationTargetIsh,
    merged: &MergedUpstream,
) -> Result<CommitOperation, RouteCommitOperationError> {
    match target {
        CommitOperationTargetIsh::Above(cli_id) => {
            let side = Side::Above;
            Ok(route_commit_above_or_below(
                repo, id_map, cli_id, side, merged,
            )?)
        }
        CommitOperationTargetIsh::Below(cli_id) => {
            let side = Side::Below;
            Ok(route_commit_above_or_below(
                repo, id_map, cli_id, side, merged,
            )?)
        }
        CommitOperationTargetIsh::Branch(cli_id) => {
            if let Some(branch) = cli_id.try_resolve_branch(repo, id_map)? {
                let segment = branch.resolve_segment(head_info)?;
                let ref_info = segment.ref_info.with_context(|| {
                    format!("BUG: Segment resolved from branch name {branch} has no ref info")
                })?;
                merged.ensure_branch_not_merged(ref_info.ref_name.as_ref())?;

                let target = CommitRelativeToTarget::BranchTip {
                    name: ref_info.ref_name,
                };

                Ok(CommitOperation::CommitAt(CommitAtOperation { target }))
            } else {
                let branch = BranchArg(cli_id.0);
                let branch_name = branch
                    .resolve_for_creation(repo, ws)
                    .with_hint(|| format!("Run `but apply {branch}` to apply the branch first"))?;
                Ok(CommitOperation::CommitToNewBranch(
                    CommitToNewBranchOperation {
                        branch_name: Some(branch_name),
                    },
                ))
            }
        }
        CommitOperationTargetIsh::UnstackedCannedBranch => Ok(CommitOperation::CommitToNewBranch(
            CommitToNewBranchOperation { branch_name: None },
        )),
        CommitOperationTargetIsh::Default => {
            // Branches that have landed upstream are not sensible default targets;
            // skip them so new work goes to a live branch or a fresh one.
            let stacks = head_info
                .stacks
                .iter()
                .filter(|stack| {
                    !stack
                        .segments
                        .first()
                        .is_some_and(|segment| merged.contains_segment(segment))
                })
                .collect::<Vec<_>>();

            match &stacks[..] {
                [] => Ok(CommitOperation::CommitToNewBranch(
                    CommitToNewBranchOperation { branch_name: None },
                )),
                [stack] => {
                    let ref_info = stack
                        .segments
                        .first()
                        .and_then(|segment| segment.ref_info.as_ref())
                        .context("Head stack has no ref")?;
                    Ok(CommitOperation::CommitAt(CommitAtOperation {
                        target: CommitRelativeToTarget::BranchTip {
                            name: ref_info.ref_name.clone(),
                        },
                    }))
                }
                stacks => {
                    let stack_heads = stacks
                        .iter()
                        .flat_map(|stack| &stack.segments)
                        .filter_map(|segment| segment.ref_info.as_ref())
                        .filter(|ref_info| {
                            merged
                                .ensure_branch_not_merged(ref_info.ref_name.as_ref())
                                .is_ok()
                        })
                        .map(|ref_info| (ref_info.ref_name.shorten(), &ref_info.ref_name))
                        .collect::<Vec<_>>();

                    let Some(stack_heads) = NonEmpty::from_vec(stack_heads) else {
                        return Err(RouteCommitOperationError::NoStackToCommitTo);
                    };

                    let Some(mut input) = out.prepare_for_terminal_input() else {
                        return Err(RouteCommitOperationError::UnclearTargetCantPrompt);
                    };

                    let mut stack_heads =
                        stack_heads.map(|(name, branch)| (name, PickerItem::Branch(branch)));
                    stack_heads.push(("Create new stack".into(), PickerItem::NewStack));

                    let Some(selection) = input.prompt_select(
                        "Multiple stacks found. Choose one to commit to",
                        &stack_heads,
                    )?
                    else {
                        return Err(bad_input("No stack picked").into());
                    };

                    match selection {
                        PickerItem::Branch(full_name) => {
                            Ok(CommitOperation::CommitAt(CommitAtOperation {
                                target: CommitRelativeToTarget::BranchTip {
                                    name: (*full_name).clone(),
                                },
                            }))
                        }
                        PickerItem::NewStack => Ok(CommitOperation::CommitToNewBranch(
                            CommitToNewBranchOperation { branch_name: None },
                        )),
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum RouteCommitOperationError {
    NoStackToCommitTo,
    UnclearTargetCantPrompt,
    Other(CliError),
}

impl From<anyhow::Error> for RouteCommitOperationError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<CliError> for RouteCommitOperationError {
    fn from(value: CliError) -> Self {
        Self::Other(value)
    }
}

impl From<BadInput> for RouteCommitOperationError {
    fn from(value: BadInput) -> Self {
        Self::Other(value.into())
    }
}

enum PickerItem<'a> {
    Branch(&'a FullName),
    NewStack,
}

fn route_commit_above_or_below(
    repo: &gix::Repository,
    id_map: &IdMap,
    cli_id: CliIdArg,
    side: Side,
    merged: &MergedUpstream,
) -> CliResult<CommitOperation> {
    let target = match cli_id
        .resolve_in_workspace(repo, id_map, Purpose::Target, None)
        .hint(
            "Target must be an applied branch or commit. Run `but status` for applicable targets.",
        )?
        .into_branch_or_commit()
        .hint("Run `but status` to show applicable targets")?
    {
        BranchOrCommit::Commit(commit) => {
            merged.ensure_commit_not_merged(commit.commit_id)?;
            CommitRelativeToTarget::Commit { commit, side }
        }
        BranchOrCommit::Branch(arg) => {
            let name = arg.resolve_local_branch_name()?;
            merged.ensure_branch_not_merged(name.as_ref())?;
            CommitRelativeToTarget::BranchBucket { name, side }
        }
    };
    Ok(CommitOperation::CommitAt(CommitAtOperation { target }))
}

pub enum CommitSelection {
    AllChanges,
    Changes(Box<NonEmpty<UncommittedHunkOrFile>>),
    Nothing,
}

pub enum CommitOperation {
    CommitToNewBranch(CommitToNewBranchOperation),
    CommitAt(CommitAtOperation),
}

impl CommitOperation {
    /// What the operation targets, for explaining rejected changes after a
    /// rollback.
    fn rejection_target(&self) -> rejection::Target {
        match self {
            CommitOperation::CommitToNewBranch(op) => rejection::Target::NewBranch(
                op.branch_name
                    .as_ref()
                    .map(|name| name.shorten().to_string()),
            ),
            CommitOperation::CommitAt(op) => match &op.target {
                CommitRelativeToTarget::Commit { commit, .. } => {
                    rejection::Target::Commit(commit.clone())
                }
                CommitRelativeToTarget::BranchTip { name } => {
                    rejection::Target::Branch(name.shorten().to_string())
                }
                CommitRelativeToTarget::BranchBucket { .. } => rejection::Target::NewBranch(None),
            },
        }
    }

    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<(IntermediateCommitCreateResult, Option<BranchNameTarget>)> {
        match self {
            CommitOperation::CommitToNewBranch(op) => op.execute(tx, changes),
            CommitOperation::CommitAt(op) => op.execute(tx, changes),
        }
    }
}

pub struct CommitToNewBranchOperation {
    pub branch_name: Option<FullName>,
}

impl CommitToNewBranchOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<(IntermediateCommitCreateResult, Option<BranchNameTarget>)> {
        let Self { branch_name } = self;

        let branch_name = if let Some(branch_name) = branch_name {
            branch_name
        } else {
            but_core::branch::unique_canned_refname(tx.repo())?
        };

        tx.create_reference(branch_name.as_ref(), None, |_| StackId::generate(), Some(0))?;

        let commit_create_result = tx.create_commit(
            RelativeTo::Reference(branch_name.clone()),
            InsertSide::Below,
            changes,
            String::new(),
            ChangeSource::Head,
        )?;

        Ok((
            commit_create_result,
            Some(BranchNameTarget::New(branch_name)),
        ))
    }
}

pub struct CommitAtOperation {
    pub target: CommitRelativeToTarget,
}

impl CommitAtOperation {
    fn execute(
        self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<(IntermediateCommitCreateResult, Option<BranchNameTarget>)> {
        let (relative_to, side, branch_name_target) = self.create_target(tx)?;

        let commit_create_result = tx.create_commit(
            relative_to.clone(),
            side,
            changes,
            String::new(),
            ChangeSource::Head,
        )?;

        Ok((commit_create_result, branch_name_target))
    }

    pub fn create_target(
        &self,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<(RelativeTo, InsertSide, Option<BranchNameTarget>)> {
        Ok(match &self.target {
            CommitRelativeToTarget::Commit { commit, side } => {
                (RelativeTo::Commit(commit.commit_id), (*side).into(), None)
            }
            CommitRelativeToTarget::BranchBucket { name, side } => {
                let new_branch_name = but_core::branch::unique_canned_refname(tx.repo())?;
                let anchor = Anchor::at_segment(name.as_ref(), (*side).into());
                tx.create_reference(
                    new_branch_name.as_ref(),
                    Some(anchor),
                    |_| StackId::generate(),
                    Some(0),
                )?;

                (
                    RelativeTo::Reference(new_branch_name.clone()),
                    InsertSide::Below,
                    Some(BranchNameTarget::New(new_branch_name)),
                )
            }
            CommitRelativeToTarget::BranchTip { name } => (
                RelativeTo::Reference(name.clone()),
                InsertSide::Below,
                Some(BranchNameTarget::Existing(name.clone())),
            ),
        })
    }
}

/// Place a commit relative to something in the workspace.
#[derive(Clone)]
pub enum CommitRelativeToTarget {
    /// Place the commit relative to this commit, within the same branch.
    Commit { commit: CommitId, side: Side },
    /// Place the commit at the tip of the branch denoted by this reference, moving the reference to
    /// the new commit. This is effectively the same as committing to a branch.
    BranchTip { name: FullName },
    /// Place the commit relative to this branch, treating the branch as a bucket.
    ///
    /// The commit is always inserted on a new branch with a canned name.
    BranchBucket { name: FullName, side: Side },
}
