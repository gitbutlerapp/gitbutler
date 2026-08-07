use std::borrow::Cow;

use anyhow::Context as _;
use but_core::{
    DryRun, RefMetadata,
    ref_metadata::StackId,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_workspace::{
    RefInfo,
    branch::create_reference::{Anchor, Position},
};
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::refs::FullName;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{BranchOrCommit, CliIdArg, Priority, Purpose},
        branch::NewPlatform,
    },
    id::CommitId,
    print_deprecation_warning,
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        merged_upstream::MergedUpstream, targeting::Side,
    },
};

pub fn new(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: NewPlatform,
) -> CliResult<NewOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let operation = {
        let head_info = but_api::legacy::workspace::head_info(ctx)?;
        resolve(ctx, guard.read_permission(), args, &head_info, &id_map)?
    };

    Ok(run(ctx, &mut meta, guard.write_permission(), operation)?)
}

fn resolve(
    ctx: &mut Context,
    perm: &RepoShared,
    args: NewPlatform,
    head_info: &RefInfo,
    id_map: &IdMap,
) -> CliResult<NewOperation> {
    let NewPlatform {
        above,
        below,
        anchor,
        name,
        allow_merged,
    } = args;

    let merged = MergedUpstream::new(&*ctx.repo.get()?, head_info, allow_merged);

    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;

    let name = name
        .map(|name| name.resolve_for_creation(&repo, &ws))
        .transpose()?;

    let above = match (above, anchor) {
        (None, None) => None,
        (None, Some(anchor)) => {
            print_deprecation_warning(
                "`--anchor/-a` is deprecated and will be removed in a future release. Use `--above/-A` instead",
            );
            Some(anchor)
        }
        (Some(above), None) => Some(above),
        (Some(_), Some(_)) => {
            unreachable!("--anchor and --above are mutually exclusive in the clap args")
        }
    };

    match (above, below) {
        (None, None) => Ok(NewOperation::NewUnstackedBranch { name }),
        (None, Some(target_below)) => {
            let target = resolve_above_below_target(&repo, id_map, target_below)?;

            match &target {
                NewStackedBranchTarget::Commit(commit) => {
                    merged.ensure_commit_not_merged(commit.commit_id)?;
                }
                NewStackedBranchTarget::Branch(target) => {
                    merged.ensure_branch_not_merged(target.as_ref())?;
                }
            }

            Ok(NewOperation::NewStackedBranch {
                name,
                target,
                side: Side::Below,
            })
        }
        (Some(target_above), None) => {
            let target = resolve_above_below_target(&repo, id_map, target_above)?;
            Ok(NewOperation::NewStackedBranch {
                name,
                target,
                side: Side::Above,
            })
        }
        (Some(_), Some(_)) => {
            unreachable!("--above and --below are mutually exclusive in the clap args")
        }
    }
}

fn resolve_above_below_target(
    repo: &gix::Repository,
    id_map: &IdMap,
    target: CliIdArg,
) -> CliResult<NewStackedBranchTarget> {
    let target = target
        .resolve_in_workspace(
            repo,
            id_map,
            Purpose::Target,
            Some(Priority::BranchAndCommit),
        )?
        .into_branch_or_commit()?;

    Ok(match target {
        BranchOrCommit::Commit(commit) => NewStackedBranchTarget::Commit(commit),
        BranchOrCommit::Branch(branch_arg) => {
            NewStackedBranchTarget::Branch(branch_arg.resolve_local_branch_name()?)
        }
    })
}

pub enum NewOperation {
    NewUnstackedBranch {
        name: Option<FullName>,
    },
    NewStackedBranch {
        name: Option<FullName>,
        target: NewStackedBranchTarget,
        side: Side,
    },
}

pub enum NewStackedBranchTarget {
    Commit(CommitId),
    Branch(FullName),
}

pub fn run(
    ctx: &mut Context,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    operation: NewOperation,
) -> anyhow::Result<NewOutcome> {
    let in_single_branch_mode = ctx.settings.feature_flags.single_branch
        && gitbutler_operating_modes::in_outside_workspace_mode(ctx, perm.read_permission())?;
    let mut checkout_after_create = false;

    let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);
    let (name, _ws) = but_transaction::with_transaction_with_perm(
        ctx,
        meta,
        perm,
        snapshot_details,
        DryRun::No,
        |mut tx| {
            let new_ref = match &operation {
                NewOperation::NewStackedBranch { name, .. }
                | NewOperation::NewUnstackedBranch { name } => {
                    if let Some(name) = name {
                        name.clone()
                    } else {
                        but_core::branch::unique_canned_refname(tx.repo())?
                    }
                }
            };

            let anchor = match &operation {
                NewOperation::NewUnstackedBranch { name: _ } => None,
                NewOperation::NewStackedBranch {
                    name: _,
                    target,
                    side,
                } => Some(match target {
                    NewStackedBranchTarget::Commit(commit_target) => Anchor::AtCommit {
                        commit_id: commit_target.commit_id,
                        position: (*side).into(),
                    },
                    NewStackedBranchTarget::Branch(branch_target) => {
                        Anchor::at_segment(branch_target.as_ref(), (*side).into())
                    }
                }),
            };

            let anchor = if let Some(anchor) = anchor {
                match anchor {
                    Anchor::AtSegment { position, ref_name }
                        if matches!(position, Position::Above) && in_single_branch_mode =>
                    {
                        let head_name = head_name(tx.repo())?;
                        if &*ref_name == head_name.as_ref() {
                            Some(single_branch_mode_anchor(
                                head_name,
                                &mut checkout_after_create,
                            )?)
                        } else {
                            Some(Anchor::AtReference { ref_name, position })
                        }
                    }
                    _ => Some(anchor),
                }
            } else if in_single_branch_mode {
                let head_name = head_name(tx.repo())?;
                Some(single_branch_mode_anchor(
                    head_name,
                    &mut checkout_after_create,
                )?)
            } else {
                None
            };

            tx.create_reference(new_ref.as_ref(), anchor, |_| StackId::generate(), Some(0))?;

            Ok(but_transaction::Commit(new_ref))
        },
    )?;

    if checkout_after_create {
        but_api::branch::branch_checkout_with_perm(ctx, name.clone(), perm)?;
    }

    let target = match operation {
        NewOperation::NewUnstackedBranch { .. } => None,
        NewOperation::NewStackedBranch { target, side, .. } => Some((target, side)),
    };

    Ok(NewOutcome { name, target })
}

fn head_name(repo: &gix::Repository) -> anyhow::Result<FullName> {
    Ok(repo
        .head()?
        .referent_name()
        .filter(|name| name.category() == Some(gix::refs::Category::LocalBranch))
        .context("single-branch branch creation requires HEAD to be a local branch")?
        .to_owned())
}

fn single_branch_mode_anchor(
    head_name: FullName,
    checkout_after_create: &mut bool,
) -> anyhow::Result<Anchor<'static>> {
    *checkout_after_create = true;

    Ok(Anchor::AtReference {
        ref_name: Cow::Owned(head_name),
        position: Side::Above.into(),
    })
}

#[must_use]
pub struct NewOutcome {
    pub name: FullName,
    pub target: Option<(NewStackedBranchTarget, Side)>,
}

impl CliOutputHuman for NewOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &Theme,
    ) -> anyhow::Result<()> {
        let Self { name, target } = self;

        write!(out, "Created branch {}", theme::Branch(name))?;

        if let Some((target, side)) = target {
            write!(out, " {side} ")?;
            match target {
                NewStackedBranchTarget::Commit(commit_target) => {
                    write!(out, "commit {}", theme::Commit(commit_target))?;
                }
                NewStackedBranchTarget::Branch(branch_target) => {
                    write!(out, "branch {}", theme::Branch(branch_target))?;
                }
            }
        }

        writeln!(out)?;

        Ok(())
    }
}

impl CliOutput for NewOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            branch: String,
        }

        let Self { name, target: _ } = self;

        Output {
            branch: name.shorten().to_string(),
        }
    }
}
