use but_ctx::Context;
use but_workspace::branch::apply::OutcomeStatus;
use gix::reference::Category;

use crate::utils::OutputChannel;

/// Apply a branch to the workspace, and return the full ref name to it.
pub fn apply(mut ctx: Context, branch_name: &str, out: &mut OutputChannel) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let reference = {
        let repo = ctx.repo.get()?;
        repo.find_reference(branch_name)?.detach()
    };
    let outcome = but_api::branch::apply_with_perm(
        &mut ctx,
        reference.name.as_ref(),
        guard.write_permission(),
    )?;

    let error_message = apply_error_message(reference.name.as_ref(), &outcome);

    if let Some(out) = out.for_human() {
        if let Some(message) = error_message.as_deref() {
            anyhow::bail!("{message}");
        }
        write_human_apply_outcome(out, reference.name.as_ref(), &outcome)?;
    } else if let Some(out) = out.for_shell() {
        write_shell_apply_outcome(out, reference.name.as_ref(), &outcome)?;
    }

    if let Some(out) = out.for_json() {
        out.write_value(but_api::branch::json::ApplyOutcome::from(outcome))?;
    }
    if let Some(message) = error_message.as_deref() {
        anyhow::bail!("{message}");
    }
    Ok(())
}

fn apply_error_message(
    requested_branch: &gix::refs::FullNameRef,
    outcome: &but_workspace::branch::apply::Outcome,
) -> Option<String> {
    if !outcome.conflicting_stacks.is_empty() {
        let short_name = requested_branch.shorten();
        let conflicting_stack_names = outcome
            .conflicting_stacks
            .iter()
            .map(|stack| stack.ref_name.shorten().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "'{short_name}' conflicts with existing stack in the workspace: {conflicting_stack_names}"
        ))
    } else if matches!(outcome.status, OutcomeStatus::ConflictAborted) {
        let short_name = requested_branch.shorten();
        Some(format!(
            "'{short_name}' could not be applied because conflicts prevented persistence"
        ))
    } else {
        None
    }
}

fn write_shell_apply_outcome(
    out: &mut dyn crate::utils::WriteWithUtils,
    requested_branch: &gix::refs::FullNameRef,
    outcome: &but_workspace::branch::apply::Outcome,
) -> std::fmt::Result {
    writeln!(out, "status={}", outcome.status.as_str())?;
    writeln!(out, "requested_branch={requested_branch}")?;
    writeln!(out, "workspace_changed={}", outcome.workspace_changed())?;
    writeln!(
        out,
        "workspace_ref_created={}",
        outcome.workspace_ref_created
    )?;
    for name in &outcome.applied_branches {
        writeln!(out, "applied_branch={name}")?;
    }
    for stack in &outcome.conflicting_stacks {
        writeln!(out, "conflicting_stack={}", stack.ref_name)?;
    }
    Ok(())
}

fn write_human_apply_outcome(
    out: &mut dyn crate::utils::WriteWithUtils,
    requested_branch: &gix::refs::FullNameRef,
    outcome: &but_workspace::branch::apply::Outcome,
) -> std::fmt::Result {
    match outcome.status {
        OutcomeStatus::AlreadyApplied => {
            let short_name = requested_branch.shorten();
            writeln!(
                out,
                "Branch '{short_name}' is already in the workspace; nothing changed"
            )?;
        }
        OutcomeStatus::Applied => {
            let mut write_applied_branch = |name: &gix::refs::FullNameRef| {
                let short_name = name.shorten();
                let is_remote_reference =
                    name.category().is_some_and(|c| c == Category::RemoteBranch);
                if is_remote_reference {
                    writeln!(out, "Applied remote branch '{short_name}' to workspace")
                } else {
                    writeln!(out, "Applied branch '{short_name}' to workspace")
                }
            };
            if outcome.applied_branches.len() == 1 {
                write_applied_branch(requested_branch)?;
            } else {
                for name in &outcome.applied_branches {
                    write_applied_branch(name.as_ref())?;
                }
            }
        }
        OutcomeStatus::ConflictAborted => {
            unreachable!("conflict-aborted applies are rejected before formatting");
        }
    }
    Ok(())
}
