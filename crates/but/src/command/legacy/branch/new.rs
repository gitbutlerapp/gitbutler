use std::borrow::Cow;

use anyhow::{Context as _, bail};
use but_core::{
    DryRun, RefMetadata,
    ref_metadata::{ProjectMeta, StackId},
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
    bad_input,
    id::CommitId,
    print_deprecation_warning,
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        in_single_branch_mode_with_perm, merged_upstream::MergedUpstream, targeting::Side,
    },
};

pub fn new(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: NewPlatform,
) -> CliResult<NewOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let operation = {
        let head_info = but_api::legacy::workspace::head_info(ctx)?;
        resolve(ctx, guard.read_permission(), args, &head_info, &id_map)?
    };

    let mut meta = ctx.meta()?;
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
        switch,
    } = args;

    if switch && !ctx.settings.feature_flags.single_branch {
        return Err(
            bad_input("`--switch` requires the `single-branch` feature to be enabled")
                .hint("Enable the feature with `but config feature single-branch enable`")
                .into(),
        );
    }

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
        (None, None) => Ok(NewOperation::NewUnstackedBranch(
            NewUnstackedBranchOperation { name, switch },
        )),
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

            Ok(NewOperation::NewStackedBranch(NewStackedBranchOperation {
                name,
                target,
                side: Side::Below,
            }))
        }
        (Some(target_above), None) => {
            let target = resolve_above_below_target(&repo, id_map, target_above)?;
            Ok(NewOperation::NewStackedBranch(NewStackedBranchOperation {
                name,
                target,
                side: Side::Above,
            }))
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
    NewUnstackedBranch(NewUnstackedBranchOperation),
    NewStackedBranch(NewStackedBranchOperation),
}

pub struct NewUnstackedBranchOperation {
    pub name: Option<FullName>,
    pub switch: bool,
}

pub struct NewStackedBranchOperation {
    pub name: Option<FullName>,
    pub target: NewStackedBranchTarget,
    pub side: Side,
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
    match operation {
        NewOperation::NewUnstackedBranch(op) => op.execute(ctx, meta, perm),
        NewOperation::NewStackedBranch(op) => op.execute(ctx, meta, perm),
    }
}

impl NewUnstackedBranchOperation {
    fn execute(
        self,
        ctx: &mut Context,
        meta: &mut impl RefMetadata,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<NewOutcome> {
        let in_single_branch_mode = in_single_branch_mode_with_perm(ctx, perm.read_permission())?;

        if in_single_branch_mode {
            self.execute_single_branch_mode(ctx, meta, perm)
        } else {
            self.execute_workspace_mode(ctx, meta, perm)
        }
    }

    fn execute_workspace_mode(
        self,
        ctx: &mut Context,
        meta: &mut impl RefMetadata,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<NewOutcome> {
        let Self { name, switch } = self;

        let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);

        let (new_ref, _ws) = but_transaction::with_transaction_with_perm(
            ctx,
            meta,
            perm,
            snapshot_details,
            DryRun::No,
            |mut tx| {
                let new_ref = if let Some(name) = name {
                    name.clone()
                } else {
                    but_core::branch::unique_canned_refname(tx.repo())?
                };

                tx.create_reference(new_ref.as_ref(), None, |_| StackId::generate(), Some(0))?;

                Ok(but_transaction::Commit(new_ref))
            },
        )?;

        if switch {
            but_api::branch::branch_checkout_with_perm(ctx, new_ref.clone(), perm)?;
        }

        Ok(NewOutcome {
            name: new_ref,
            target: None,
        })
    }

    fn execute_single_branch_mode(
        self,
        ctx: &mut Context,
        meta: &mut impl RefMetadata,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<NewOutcome> {
        let Self { name, switch } = self;

        let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);

        let repo = ctx.repo.get()?;
        let project_meta = ProjectMeta::resolve(&repo)?;
        let head_name = head_name(&repo)?;

        let new_ref = if let Some(name) = name {
            name.clone()
        } else {
            but_core::branch::unique_canned_refname(&repo)?
        };

        let target_ref = project_meta
            .target_ref
            .as_ref()
            .context("BUG: target ref is missing")?;

        let is_on_target =
            but_core::branch::resolve_tracking_branch_ref_name(head_name.as_ref(), &repo)
                .is_ok_and(|upstream| &*upstream == target_ref.as_ref());

        if is_on_target {
            // we're directly on the target then we haven't created any branches yet so
            // create the branch on top of the target then check it out

            drop(repo);

            but_transaction::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
                DryRun::No,
                |mut tx| {
                    let anchor = Some(Anchor::AtReference {
                        ref_name: Cow::Owned(head_name),
                        position: Side::Above.into(),
                    });

                    tx.create_reference(
                        new_ref.as_ref(),
                        anchor,
                        |_| StackId::generate(),
                        Some(0),
                    )?;

                    Ok(())
                },
            )?;

            but_api::branch::branch_checkout_with_perm(ctx, new_ref.clone(), perm)?;
        } else if switch {
            drop(repo);

            let target_commit_id = project_meta.target_commit_id_or_err()?;

            but_transaction::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
                DryRun::No,
                |tx| {
                    tx.repo().reference(
                        new_ref.as_ref(),
                        target_commit_id,
                        gix::refs::transaction::PreviousValue::MustNotExist,
                        format!("create {new_ref}"),
                    )?;

                    Ok(())
                },
            )?;

            but_api::branch::branch_checkout_with_perm(ctx, new_ref.clone(), perm)?;
        } else {
            // if we're not on the target then enter a workspace and create the branch

            if repo
                .try_find_reference(but_core::WORKSPACE_REF_NAME)?
                .is_none()
            {
                // the workspace doesn't exist, create it
                drop(repo);
                let target_ref = target_ref.to_string().parse()?;
                gitbutler_branch_actions::set_base_branch(ctx, &target_ref, perm)?;
            } else {
                drop(repo);
            }

            // make sure the previous branch is applied
            // if the branch had no commits `set_base_branch` doesn't apply it
            //
            // this also has the effect of entering the workspace with one branch applied
            {
                let (repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
                let outcome = but_workspace::branch::apply(
                    head_name.as_ref(),
                    ws.clone(),
                    &repo,
                    meta,
                    but_workspace::branch::apply::Options {
                        allow_applying_already_applied_branch_when_outside_workspace: true,
                        ..Default::default()
                    },
                )?;
                if outcome.status.persisted_mutation() {
                    *ws = outcome.workspace.clone();
                } else {
                    bail!(
                        "BUG: failed to apply head ref ({head_name}). Failed with {:?}",
                        outcome.status
                    )
                }
            };

            but_transaction::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
                DryRun::No,
                |mut tx| {
                    tx.create_reference(new_ref.as_ref(), None, |_| StackId::generate(), Some(0))?;

                    Ok(())
                },
            )?;
        }

        Ok(NewOutcome {
            name: new_ref,
            target: None,
        })
    }
}

impl NewStackedBranchOperation {
    fn execute(
        self,
        ctx: &mut Context,
        meta: &mut impl RefMetadata,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<NewOutcome> {
        let Self { name, target, side } = self;

        let in_single_branch_mode = in_single_branch_mode_with_perm(ctx, perm.read_permission())?;

        let mut checkout_after_create = false;

        let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);

        let (new_ref, _ws) = but_transaction::with_transaction_with_perm(
            ctx,
            meta,
            perm,
            snapshot_details,
            DryRun::No,
            |mut tx| {
                let new_ref = if let Some(name) = name {
                    name.clone()
                } else {
                    but_core::branch::unique_canned_refname(tx.repo())?
                };

                let anchor = match &target {
                    NewStackedBranchTarget::Commit(commit_target) => Anchor::AtCommit {
                        commit_id: commit_target.commit_id,
                        position: side.into(),
                    },
                    NewStackedBranchTarget::Branch(branch_target) => {
                        Anchor::at_segment(branch_target.as_ref(), side.into())
                    }
                };

                let anchor = if in_single_branch_mode
                    && let Anchor::AtSegment {
                        position: position @ Position::Above,
                        ref_name,
                    } = anchor
                {
                    // creating a new branch above HEAD works differently in single branch mode
                    // have to use a different anchor type and manually checkout the newly created
                    // branch
                    let head_name = head_name(tx.repo())?;
                    if &*ref_name == head_name.as_ref() {
                        checkout_after_create = true;
                        Anchor::AtReference {
                            ref_name: Cow::Owned(head_name),
                            position: Side::Above.into(),
                        }
                    } else {
                        Anchor::AtReference { ref_name, position }
                    }
                } else {
                    anchor
                };

                tx.create_reference(
                    new_ref.as_ref(),
                    anchor.clone(),
                    |_| StackId::generate(),
                    Some(0),
                )
                .with_context(|| {
                    format!("failed to create reference. anchor={anchor:?}; new_ref={new_ref:?}")
                })?;

                Ok(but_transaction::Commit(new_ref))
            },
        )?;

        if checkout_after_create {
            but_api::branch::branch_checkout_with_perm(ctx, new_ref.clone(), perm)?;
        }

        Ok(NewOutcome {
            name: new_ref,
            target: Some((target, side)),
        })
    }
}

fn head_name(repo: &gix::Repository) -> anyhow::Result<FullName> {
    Ok(repo
        .head()?
        .referent_name()
        .filter(|name| name.category() == Some(gix::refs::Category::LocalBranch))
        .context("single-branch branch creation requires HEAD to be a local branch")?
        .to_owned())
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
