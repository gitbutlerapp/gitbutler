use anyhow::Context as _;
use but_core::{
    RefMetadata,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_transaction::single_branch_mode::SingleBranchMode;
use but_workspace::{RefInfo, branch::create_reference::Anchor};
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
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, change_source::InvokedFrom,
        merged_upstream::MergedUpstream, name_new_branch, targeting::Side,
    },
};

pub fn new(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: NewPlatform,
    invoked_from: &InvokedFrom,
) -> CliResult<NewOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission(), Some(invoked_from))?;

    let operation = {
        let head_info = but_api::legacy::workspace::head_info(ctx)?;
        resolve(
            ctx,
            guard.read_permission(),
            args,
            &head_info,
            &id_map,
            invoked_from,
        )?
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
    invoked_from: &InvokedFrom,
) -> CliResult<NewOperation> {
    let NewPlatform {
        above,
        below,
        name,
        allow_merged,
        switch,
    } = args;

    let merged = MergedUpstream::new(
        &*ctx.repo.get()?,
        &ctx.project_meta()?,
        head_info,
        allow_merged,
    );

    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;

    let name = name
        .map(|name| name.resolve_for_creation(&repo, &ws))
        .transpose()?;

    match (above, below) {
        (None, None) => {
            if let Some(worktree) = &invoked_from.linked_worktree {
                return Err(bad_input(format!(
                    "Cannot create an unstacked branch from worktree {worktree}"
                ))
                .hint("Use `--above` or `--below` to place it, or `but worktree new` for a new worktree")
                .into());
            }
            Ok(NewOperation::NewUnstackedBranch(
                NewUnstackedBranchOperation { name, switch },
            ))
        }
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
                switch,
            }))
        }
        (Some(target_above), None) => {
            let target = resolve_above_below_target(&repo, id_map, target_above)?;
            Ok(NewOperation::NewStackedBranch(NewStackedBranchOperation {
                name,
                target,
                side: Side::Above,
                switch,
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
    pub switch: bool,
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
        let NewUnstackedBranchOperation { mut name, switch } = self;
        let sbm = SingleBranchMode::new(ctx, perm.read_permission(), switch)?;
        let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);
        let new_ref = name_new_branch(&mut name, &*ctx.repo.get()?)?;

        // Named so undo deletes it again.
        sbm.transaction_with_workspace_setup(
            ctx,
            meta,
            snapshot_details,
            &[new_ref.as_ref()],
            perm,
            true,
            |mut tx| sbm.create_unstacked_reference(&mut tx, new_ref.as_ref(), Some(0)),
        )?;

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
        let NewStackedBranchOperation {
            mut name,
            target,
            side,
            switch,
        } = self;

        let sbm = SingleBranchMode::new(ctx, perm.read_permission(), switch)?;
        let snapshot_details = SnapshotDetails::new(OperationKind::CreateBranch);
        let new_ref = name_new_branch(&mut name, &*ctx.repo.get()?)?;

        // Named so undo deletes it again.
        sbm.transaction_with_workspace_setup(
            ctx,
            meta,
            snapshot_details,
            &[new_ref.as_ref()],
            perm,
            false,
            |mut tx| {
                match &target {
                    NewStackedBranchTarget::Commit(commit_target) => {
                        let anchor = Anchor::AtCommit {
                            commit_id: commit_target.commit_id,
                            position: side.into(),
                        };
                        tx.create_reference(new_ref.as_ref(), anchor.clone(), Some(0))
                            .with_context(|| {
                                format!(
                                    "failed to create reference. anchor={anchor:?}; new_ref={new_ref:?}"
                                )
                            })?;
                    }
                    NewStackedBranchTarget::Branch(branch_target) => {
                        sbm.create_stacked_reference(
                            &mut tx,
                            new_ref.as_ref(),
                            branch_target.as_ref(),
                            side.into(),
                        )
                        .with_context(|| {
                            format!(
                                "failed to create reference. relative_to={branch_target:?}; side={side}; new_ref={new_ref:?}"
                            )
                        })?;
                    }
                }

                if switch {
                    tx.checkout(new_ref.as_ref())?;
                }

                Ok(())
            },
        )?;

        Ok(NewOutcome {
            name: new_ref,
            target: Some((target, side)),
        })
    }
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
