use but_core::{
    WORKSPACE_REF_NAME,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use gix::refs::FullName;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{atoms::BranchArg, r#switch::Platform},
    command::legacy::branch::{
        self,
        new::{NewOperation, NewUnstackedBranchOperation},
    },
    print_deprecation_warning,
    theme::{self, Theme},
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

pub fn switch(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<SwitchOutcome> {
    let mut guard = ctx.exclusive_worktree_access();

    if args.new {
        print_deprecation_warning(
            "`--new/-n` is deprecated and will be removed in a future release. Use `but branch new --switch` instead",
        );
    }

    let operation = resolve(ctx, guard.read_permission(), args)?;

    Ok(run(ctx, guard.write_permission(), operation)?)
}

fn resolve(ctx: &Context, perm: &RepoShared, args: Platform) -> CliResult<SwitchOperation> {
    let Platform {
        target,
        workspace,
        new,
    } = args;

    if workspace {
        return Ok(SwitchOperation::Workspace);
    }

    if new {
        let name = target
            .map(|target| {
                let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
                BranchArg(target.0).resolve_for_creation(&repo, &ws)
            })
            .transpose()?;

        return Ok(SwitchOperation::NewBranch { name });
    }

    let target = target
        .ok_or_else(|| anyhow::anyhow!("BUG: clap requires target, --workspace, or --new"))?;
    let branch = {
        let repo = ctx.repo.get()?;
        let id_map = IdMap::new_from_context(ctx, perm)?;
        target.resolve_existing_local_branch(&repo, &id_map)?
    };

    Ok(SwitchOperation::Branch { branch })
}

pub fn run(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    operation: SwitchOperation,
) -> anyhow::Result<SwitchOutcome> {
    match operation {
        SwitchOperation::Workspace => {
            let workspace_exists = {
                let repo = ctx.repo.get()?;
                repo.try_find_reference(WORKSPACE_REF_NAME)?.is_some()
            };
            if workspace_exists {
                but_api::branch::workspace_checkout_with_perm(ctx, perm)?;
            } else {
                but_api::legacy::virtual_branches::switch_back_to_workspace_with_perm(ctx, perm)?;
            }

            Ok(SwitchOutcome::Workspace)
        }
        SwitchOperation::Branch { branch } => {
            but_api::branch::branch_checkout_with_perm(ctx, branch.clone(), perm)?;

            Ok(SwitchOutcome::Branch { branch })
        }
        SwitchOperation::NewBranch { name } => {
            let mut meta = ctx.meta()?;
            let outcome = branch::new::run(
                ctx,
                &mut meta,
                perm,
                NewOperation::NewUnstackedBranch(NewUnstackedBranchOperation {
                    name,
                    switch: true,
                }),
            )?;

            Ok(SwitchOutcome::CreatedBranch {
                branch: outcome.name,
            })
        }
    }
}

pub enum SwitchOperation {
    Workspace,
    Branch { branch: FullName },
    NewBranch { name: Option<FullName> },
}

#[must_use]
pub enum SwitchOutcome {
    Workspace,
    Branch { branch: FullName },
    CreatedBranch { branch: FullName },
}

impl CliOutputHuman for SwitchOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            SwitchOutcome::Workspace => writeln!(out, "Switched to workspace")?,
            SwitchOutcome::Branch { branch } => {
                writeln!(out, "Switched to branch {}", theme::Branch(branch))?
            }
            SwitchOutcome::CreatedBranch { branch } => {
                writeln!(out, "Created branch {}", theme::Branch(branch))?
            }
        }

        Ok(())
    }
}

impl CliOutput for SwitchOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        struct Output {
            branch: String,
        }

        match self {
            SwitchOutcome::Workspace | SwitchOutcome::Branch { .. } => None,
            SwitchOutcome::CreatedBranch { branch } => Some(Output {
                branch: branch.shorten().to_string(),
            }),
        }
    }
}
