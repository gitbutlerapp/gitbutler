//! Implementation of the `but apply` command.

use anyhow::Context as _;
use but_core::sync::RepoExclusive;
use but_ctx::Context;
use but_workspace::branch::apply::OutcomeStatus;
use gix::refs::{Category, FullName};
use serde::Serialize;

use crate::{
    CliResult,
    args::apply::Platform,
    theme::{self, Theme},
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

pub enum ApplyOutcome {
    AlreadyApplied {
        requested_branch: FullName,
        outcome: but_workspace::branch::apply::Outcome,
    },
    Applied {
        requested_branch: FullName,
        outcome: but_workspace::branch::apply::Outcome,
    },
    ConflictAborted(ConflictAbortedOutcome),
}

impl CliOutputHuman for ApplyOutcome {
    fn is_rejection(&self) -> bool {
        match self {
            ApplyOutcome::ConflictAborted(..) => true,
            ApplyOutcome::AlreadyApplied { .. } | ApplyOutcome::Applied { .. } => false,
        }
    }

    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            ApplyOutcome::AlreadyApplied {
                requested_branch: branch,
                outcome: _,
            } => {
                writeln!(
                    out,
                    "Branch {} is already in the workspace; nothing changed",
                    theme::Branch(branch),
                )?;
            }
            ApplyOutcome::Applied {
                requested_branch,
                outcome,
            } => {
                fn write_applied_branch(
                    out: &mut dyn WriteWithUtils,
                    name: &gix::refs::FullNameRef,
                ) -> anyhow::Result<()> {
                    if name.category().is_some_and(|c| c == Category::RemoteBranch) {
                        writeln!(
                            out,
                            "Applied remote branch {} to workspace",
                            theme::Branch(name)
                        )?;
                    } else {
                        writeln!(out, "Applied branch {} to workspace", theme::Branch(name))?;
                    }

                    Ok(())
                }

                let applied_branches = outcome.applied_branches;

                if applied_branches.len() == 1 {
                    write_applied_branch(out, requested_branch.as_ref())?;
                } else {
                    for name in applied_branches {
                        write_applied_branch(out, name.as_ref())?;
                    }
                }
            }
            ApplyOutcome::ConflictAborted(outcome) => {
                writeln!(out, "Failed to apply branch: {outcome}")?;
            }
        }

        Ok(())
    }
}

impl CliOutput for ApplyOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            status: OutcomeStatus,
            workspace_changed: bool,
            applied_branches: Vec<String>,
            workspace_ref_created: bool,
            conflicting_stacks: Vec<String>,
        }

        impl From<but_workspace::branch::apply::Outcome> for Output {
            fn from(value: but_workspace::branch::apply::Outcome) -> Self {
                Self {
                    status: value.status,
                    workspace_changed: value.workspace_changed(),
                    applied_branches: value
                        .applied_branches
                        .into_iter()
                        .map(|branch| branch.to_string())
                        .collect(),
                    workspace_ref_created: value.workspace_ref_created,
                    conflicting_stacks: value
                        .conflicting_stacks
                        .into_iter()
                        .map(|stack| stack.ref_name.to_string())
                        .collect(),
                }
            }
        }

        let outcome = match self {
            ApplyOutcome::AlreadyApplied { outcome, .. }
            | ApplyOutcome::ConflictAborted(ConflictAbortedOutcome { outcome, .. })
            | ApplyOutcome::Applied { outcome, .. } => outcome,
        };

        Output::from(outcome)
    }
}

pub struct ConflictAbortedOutcome {
    pub requested_branch: FullName,
    pub outcome: but_workspace::branch::apply::Outcome,
}

impl std::fmt::Display for ConflictAbortedOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.outcome.conflicting_stacks.is_empty() {
            let short_name = self.requested_branch.shorten();
            let conflicting_stack_names = self
                .outcome
                .conflicting_stacks
                .iter()
                .map(|stack| stack.ref_name.shorten().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            write!(
                f,
                "'{short_name}' conflicts with existing stack: {conflicting_stack_names}"
            )
        } else if matches!(self.outcome.status, OutcomeStatus::ConflictAborted) {
            let short_name = self.requested_branch.shorten();
            write!(
                f,
                "'{short_name}' could not be applied because it would cause conflicts"
            )
        } else {
            Ok(())
        }
    }
}

pub fn apply(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<ApplyOutcome> {
    let mut guard = ctx.exclusive_worktree_access();

    let operation = {
        let repo = ctx.repo.get()?;
        resolve(args, &repo).context("Failed to apply branch")?
    };

    Ok(run(ctx, guard.write_permission(), operation).context("Failed to apply branch")?)
}

fn resolve(args: Platform, repo: &gix::Repository) -> anyhow::Result<ApplyOperation> {
    let Platform { branch } = args;

    let branch = branch.resolve_legacy_top_level_apply_branch_name(repo)?;

    Ok(ApplyOperation { branch })
}

pub fn run(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    operation: ApplyOperation,
) -> anyhow::Result<ApplyOutcome> {
    let ApplyOperation { branch } = operation;

    let reference = {
        let repo = ctx.repo.get()?;
        repo.find_reference(&*branch)?.detach()
    };

    let outcome = but_api::branch::apply_with_perm(ctx, reference.name.as_ref(), perm)?;

    if !outcome.conflicting_stacks.is_empty() {
        return Ok(ApplyOutcome::ConflictAborted(ConflictAbortedOutcome {
            requested_branch: reference.name,
            outcome,
        }));
    }

    match outcome.status {
        OutcomeStatus::Applied => Ok(ApplyOutcome::Applied {
            requested_branch: reference.name,
            outcome,
        }),
        OutcomeStatus::AlreadyApplied => Ok(ApplyOutcome::AlreadyApplied {
            requested_branch: reference.name,
            outcome,
        }),
        OutcomeStatus::ConflictAborted => {
            Ok(ApplyOutcome::ConflictAborted(ConflictAbortedOutcome {
                requested_branch: reference.name,
                outcome,
            }))
        }
    }
}

pub struct ApplyOperation {
    pub branch: String,
}
