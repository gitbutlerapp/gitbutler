use but_api::json::{ChangeIdString, HexHash};
use but_core::{
    DryRun, RefMetadata,
    ref_metadata::StackId,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_workspace::RefInfo;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::ObjectId;
use itertools::Itertools as _;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{CommitArg, Priority, Purpose, ResolvedCliIdArg},
        pick::Platform,
    },
    bad_input,
    command::legacy::commit::{
        BranchNameTarget, CommitOperation, CommitOperationTargetIsh, CommitToNewBranchOperation,
        RouteCommitOperationError, route_commit_operation,
    },
    id::CommitId,
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        merged_upstream::MergedUpstream,
    },
};

#[derive(Debug)]
pub struct PickOutcome {
    pub sources: Vec<ObjectId>,
    pub new_commits: Vec<CommitId>,
    pub branch_name: Option<BranchNameTarget>,
}

impl CliOutputHuman for PickOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &Theme,
    ) -> anyhow::Result<()> {
        let Self {
            sources,
            new_commits,
            branch_name,
        } = self;

        let sources = sources
            .into_iter()
            .map(|commit| CommitId {
                commit_id: commit,
                change_id: None,
            })
            .map(theme::Commit)
            .join(", ");
        let new_commits = new_commits.into_iter().map(theme::Commit).join(", ");

        match branch_name {
            Some(BranchNameTarget::New(branch_name)) => writeln!(
                out,
                "Picked {} onto new branch {} to create {}",
                sources,
                theme::Branch(branch_name),
                new_commits,
            )?,
            Some(BranchNameTarget::Existing(branch_name)) => writeln!(
                out,
                "Picked {} onto branch {} to create {}",
                sources,
                theme::Branch(branch_name),
                new_commits,
            )?,
            None => writeln!(out, "Picked {sources} to create {new_commits}")?,
        }

        Ok(())
    }
}

impl CliOutput for PickOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct PickedCommit {
            source_commit_id: HexHash,
            new_commit_id: HexHash,
            #[serde(skip_serializing_if = "Option::is_none")]
            new_change_id: Option<ChangeIdString>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            commits: Vec<PickedCommit>,
            #[serde(skip_serializing_if = "Option::is_none")]
            branch: Option<String>,
        }

        let Self {
            sources,
            new_commits,
            branch_name,
        } = self;

        Output {
            commits: sources
                .into_iter()
                .zip(new_commits)
                .map(|(source, new)| PickedCommit {
                    source_commit_id: source.into(),
                    new_commit_id: new.commit_id.into(),
                    new_change_id: new.change_id.map(Into::into),
                })
                .collect(),
            branch: match branch_name {
                Some(BranchNameTarget::New(branch_name)) => Some(branch_name.shorten().to_string()),
                Some(BranchNameTarget::Existing(_)) | None => None,
            },
        }
    }
}

pub fn pick(
    ctx: &mut Context,
    mut out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<PickOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let head_info = but_api::legacy::workspace::head_info(ctx)?;

    let pick_op = resolve(
        ctx,
        &head_info,
        &mut out,
        guard.read_permission(),
        &id_map,
        args,
    )?;

    Ok(run(ctx, &mut meta, guard.write_permission(), pick_op)?)
}

fn resolve(
    ctx: &Context,
    head_info: &RefInfo,
    out: &mut IntermediateChannel<'_>,
    perm: &RepoShared,
    id_map: &IdMap,
    args: Platform,
) -> CliResult<PickOperation> {
    let Platform {
        branch,
        above,
        below,
        sources,
        allow_merged,
    } = args;

    let merged = MergedUpstream::new(&*ctx.repo.get()?, head_info, allow_merged);

    let sources = {
        let repo = ctx.repo.get()?;
        sources
            .into_iter()
            .map(|source| {
                if let Some(resolved) =
                    source.try_resolve(&repo, id_map, Purpose::Source, Some(Priority::Commit))?
                {
                    match resolved {
                        ResolvedCliIdArg::Commit(commit_id) => Ok(commit_id.commit_id),
                        ResolvedCliIdArg::Branch(..)
                        | ResolvedCliIdArg::UncommittedHunkOrFile(..)
                        | ResolvedCliIdArg::CommittedFile(..)
                        | ResolvedCliIdArg::Uncommitted
                        | ResolvedCliIdArg::PathPrefix { .. }
                        | ResolvedCliIdArg::Stack { .. } => Err(bad_input(format!(
                            "Only commits can be cherry-picked. {} is {}",
                            source,
                            resolved.kind_for_humans()
                        ))
                        .into()),
                    }
                } else {
                    CommitArg(source.0).resolve(&repo)
                }
            })
            .collect::<CliResult<Vec<_>>>()?
            .into_iter()
            .unique()
            .collect()
    };

    let target_ish = CommitOperationTargetIsh::resolve(branch, above, below)?;

    let commit_op = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
        route_commit_operation(&repo, &ws, head_info, out, id_map, target_ish, &merged).map_err(
            |err| match err {
                RouteCommitOperationError::NoStackToCommitTo => {
                    bad_input("Found no stack that could be picked to").into()
                }
                RouteCommitOperationError::UnclearTargetCantPrompt => {
                    bad_input("Unclear where to pick to. Found more than one stack")
                        .hint("You can specify where to pick to with `--branch [<BRANCH>]`")
                        .into()
                }
                RouteCommitOperationError::Other(cli_error) => cli_error,
            },
        )?
    };

    Ok(PickOperation {
        sources,
        commit_op,
        order_commits_by_parentage: false,
    })
}

pub struct PickOperation {
    pub sources: Vec<ObjectId>,
    pub commit_op: CommitOperation,
    pub order_commits_by_parentage: bool,
}

pub fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    pick_op: PickOperation,
) -> anyhow::Result<PickOutcome> {
    let PickOperation {
        sources,
        commit_op,
        order_commits_by_parentage,
    } = pick_op;

    let snapshot_details =
        SnapshotDetails::new(OperationKind::CherryPick).with_count(sources.len());
    let ((new_commits, branch_name_target), _ws) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let (new_commits, branch_name_target) = match commit_op {
                CommitOperation::CommitToNewBranch(CommitToNewBranchOperation { branch_name }) => {
                    let branch_name = if let Some(branch_name) = branch_name {
                        branch_name
                    } else {
                        but_core::branch::unique_canned_refname(tx.repo())?
                    };

                    tx.create_reference(
                        branch_name.as_ref(),
                        None,
                        |_| StackId::generate(),
                        Some(0),
                    )?;

                    let new_commits = tx.cherry_pick_commits(
                        sources.iter().copied(),
                        RelativeTo::Reference(branch_name.clone()),
                        InsertSide::Below,
                        order_commits_by_parentage,
                    )?;

                    (new_commits, Some(BranchNameTarget::New(branch_name)))
                }
                CommitOperation::CommitAt(op) => {
                    let (relative_to, side, branch_name_target) = op.create_target(&mut tx)?;
                    let new_commits = tx.cherry_pick_commits(
                        sources.iter().copied(),
                        relative_to,
                        side,
                        order_commits_by_parentage,
                    )?;

                    (new_commits, branch_name_target)
                }
            };

            Ok(but_transaction::Commit((new_commits, branch_name_target)))
        },
    )?;

    let new_commits = new_commits.into_iter().map(Into::into).collect();

    Ok(PickOutcome {
        sources,
        new_commits,
        branch_name: branch_name_target,
    })
}
