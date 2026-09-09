use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
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
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, head_name},
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
                recreate_workspace(ctx, perm)
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

/// Recreate the workspace by applying all previously applied branches.
///
/// TODO(david): move this into but-api and expose to Lite SDK.
fn recreate_workspace(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
) -> anyhow::Result<SwitchOutcome> {
    // TODO(david): only create the snapshot if we're not already on the workspace, i.e. we don't
    // return `SwitchOutcome::AlreadyOnWorkspace`
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::SwitchToWorkspace),
        perm.read_permission(),
        but_core::DryRun::No,
    );

    let outcome = {
        let mut meta = ctx.meta()?;
        let (repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;

        let previously_applied_stack_heads: Vec<FullName> = {
            let workspace_ref: FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
            let workspace_meta = meta.workspace(workspace_ref.as_ref())?;
            workspace_meta
                .stack_names(but_core::ref_metadata::StackKind::Applied)
                .map(|name| name.to_owned())
                .collect::<Vec<_>>()
        };

        match &ws.kind {
            but_graph::workspace::WorkspaceKind::Managed { .. }
            | but_graph::workspace::WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => {
                return Ok(SwitchOutcome::AlreadyOnWorkspace);
            }
            but_graph::workspace::WorkspaceKind::AdHoc => {}
        }

        let head_name = head_name(&repo)?;

        if !ws.is_branch_the_target_or_its_local_tracking_branch(head_name.as_ref()) {
            // applying the current branch has the effect of entering the workspace with one branch applied
            let outcome = but_workspace::branch::apply(
                head_name.as_ref(),
                ws.clone(),
                &repo,
                &mut meta,
                but_workspace::branch::apply::Options {
                    allow_applying_already_applied_branch_when_outside_workspace: true,
                    ..Default::default()
                },
            )?;
            if outcome.status.persisted_mutation() {
                *ws = outcome.workspace.clone();
            } else {
                anyhow::bail!(
                    "BUG: failed to apply head ref ({head_name}). Failed with {:?}",
                    outcome.status
                )
            }
        }

        if previously_applied_stack_heads.is_empty() {
            drop((repo, ws, _db));
            but_api::branch::workspace_checkout_with_perm_only(ctx, perm)?;

            SwitchOutcome::Workspace {
                conflicting_stacks: Vec::new(),
            }
        } else {
            let mut conflicting_stacks = Vec::new();

            // apply all previously applied branches
            for stack_ref in previously_applied_stack_heads {
                let apply_outcome = but_workspace::branch::apply(
                    stack_ref.as_ref(),
                    ws.clone(),
                    &repo,
                    &mut meta,
                    but_workspace::branch::apply::Options::default(),
                )?;

                if !apply_outcome.conflicting_stacks.is_empty() {
                    conflicting_stacks.push(stack_ref);
                }

                if apply_outcome.status.persisted_mutation() {
                    *ws = apply_outcome.workspace.clone();
                }
            }

            SwitchOutcome::Workspace { conflicting_stacks }
        }
    };

    if let Some(snapshot) = maybe_oplog_entry {
        _ = snapshot.commit(ctx, perm);
    }

    Ok(outcome)
}
