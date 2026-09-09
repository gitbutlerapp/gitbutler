use but_core::{
    WORKSPACE_REF_NAME,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use gix::refs::FullName;
use itertools::Itertools as _;
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
                let result = but_api::workspace::workspace_recreate_with_perm(ctx, perm)?;
                if result.already_on_workspace {
                    Ok(SwitchOutcome::AlreadyOnWorkspace)
                } else {
                    Ok(SwitchOutcome::Workspace {
                        conflicting_stacks: result.conflicting_stacks,
                    })
                }
            } else {
                but_api::legacy::virtual_branches::switch_back_to_workspace_with_perm(ctx, perm)?;
                Ok(SwitchOutcome::Workspace {
                    conflicting_stacks: Default::default(),
                })
            }
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
    Workspace { conflicting_stacks: Vec<FullName> },
    AlreadyOnWorkspace,
    Branch { branch: FullName },
    CreatedBranch { branch: FullName },
}

impl CliOutputHuman for SwitchOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            SwitchOutcome::Workspace { conflicting_stacks } => {
                writeln!(out, "Switched to workspace")?;

                if !conflicting_stacks.is_empty() {
                    writeln!(out)?;
                    writeln!(
                        out,
                        "{} Failed to apply {} due to conflicts with existing {}",
                        theme.sym().warning,
                        conflicting_stacks.iter().map(theme::Branch).join(", "),
                        if conflicting_stacks.len() == 1 {
                            "stack"
                        } else {
                            "stacks"
                        },
                    )?;
                }
            }
            SwitchOutcome::AlreadyOnWorkspace => {
                writeln!(out, "Already on workspace")?;
            }
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
        #[serde(
            tag = "type",
            rename_all = "camelCase",
            rename_all_fields = "camelCase"
        )]
        enum Output {
            CreatedBranch { branch: String },
            SwitchedToWorkspace { conflicting_branches: Vec<String> },
        }

        match self {
            SwitchOutcome::Workspace { conflicting_stacks } => Some(Output::SwitchedToWorkspace {
                conflicting_branches: conflicting_stacks
                    .into_iter()
                    .map(|b| b.shorten().to_string())
                    .collect(),
            }),
            SwitchOutcome::AlreadyOnWorkspace => Some(Output::SwitchedToWorkspace {
                conflicting_branches: Default::default(),
            }),
            SwitchOutcome::Branch { .. } => None,
            SwitchOutcome::CreatedBranch { branch } => Some(Output::CreatedBranch {
                branch: branch.shorten().to_string(),
            }),
        }
    }
}
