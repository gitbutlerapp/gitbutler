//! Implementation of the `but unapply` command.

use anyhow::{Context as _, bail};
use but_core::ref_metadata::StackId;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails, Trailer},
};

use crate::{
    CliId, IdMap,
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

/// Handle the unapply command.
///
/// The identifier can be:
/// - A CLI ID pointing to a stack
/// - A CLI ID pointing to a branch
/// - A branch name
///
/// If a branch is specified, the entire stack containing that branch will be unapplied.
pub fn handle(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    identifier: &str,
    force: bool,
) -> anyhow::Result<()> {
    // Fetch stacks once at the start
    let stacks = but_api::legacy::workspace::stacks(ctx, Some(but_workspace::legacy::StacksFilter::InWorkspace))?;

    let id_map = IdMap::new_from_context(ctx, None)?;
    let parsed_ids = id_map.parse_using_context(identifier, ctx)?;

    // Try to find the stack to unapply
    let (stack_id, branches) = if parsed_ids.is_empty() {
        // No CLI ID match, try to find by branch name directly
        find_stack_by_branch_name(&stacks, identifier)?
    } else if parsed_ids.len() == 1 {
        match &parsed_ids[0] {
            CliId::Stack { stack_id, .. } => {
                // Direct stack ID - get the branches for display
                get_stack_branches(&stacks, *stack_id, identifier)?
            }
            CliId::Branch { name, stack_id, .. } => {
                // Branch ID - use the associated stack
                if let Some(stack_id) = stack_id {
                    get_stack_branches(&stacks, *stack_id, name)?
                } else {
                    bail!("Branch '{}' does not have an associated stack", name);
                }
            }
            CliId::Commit { .. } => {
                bail!("Cannot unapply a commit. Please specify a branch or stack identifier.");
            }
            CliId::Uncommitted(_) | CliId::CommittedFile { .. } => {
                bail!("Cannot unapply a file. Please specify a branch or stack identifier.");
            }
            CliId::Unassigned { .. } => {
                bail!("Cannot unapply the unassigned area. Please specify a branch or stack identifier.");
            }
        }
    } else {
        bail!(
            "Ambiguous identifier '{}', matches {} items. Please be more specific.",
            identifier,
            parsed_ids.len()
        );
    };

    confirm_and_unapply_stack(ctx, stack_id, &branches, force, out)
}

/// Get branches for a stack by ID, validating the stack exists.
fn get_stack_branches(
    stacks: &[but_workspace::legacy::ui::StackEntry],
    stack_id: StackId,
    identifier: &str,
) -> anyhow::Result<(StackId, Vec<String>)> {
    let stack = stacks
        .iter()
        .find(|s| s.id == Some(stack_id))
        .with_context(|| format!("Stack for '{}' not found in workspace", identifier))?;

    let branches: Vec<String> = stack.heads.iter().map(|h| h.name.to_string()).collect();

    if branches.is_empty() {
        bail!("Stack for '{}' has no branches", identifier);
    }

    Ok((stack_id, branches))
}

/// Find a stack by branch name and return the stack ID and branches.
fn find_stack_by_branch_name(
    stacks: &[but_workspace::legacy::ui::StackEntry],
    branch_name: &str,
) -> anyhow::Result<(StackId, Vec<String>)> {
    for stack_entry in stacks {
        if stack_entry.heads.iter().any(|b| b.name == branch_name)
            && let Some(sid) = stack_entry.id
        {
            let branches: Vec<String> = stack_entry.heads.iter().map(|h| h.name.to_string()).collect();
            return Ok((sid, branches));
        }
    }

    bail!("Branch '{}' not found in any applied stack", branch_name);
}

/// Create a snapshot in the oplog before performing an unapply operation
fn create_snapshot(ctx: &mut but_ctx::Context, branches: &[String]) {
    let mut guard = ctx.exclusive_worktree_access();

    // Create trailers with branch names
    let trailers: Vec<Trailer> = branches
        .iter()
        .map(|name| Trailer {
            key: "branch".to_string(),
            value: name.clone(),
        })
        .collect();

    let details = SnapshotDetails::new(OperationKind::UnapplyBranch).with_trailers(trailers);
    let _snapshot = ctx.create_snapshot(details, guard.write_permission()).ok();
}

/// Confirm with the user and unapply the stack.
fn confirm_and_unapply_stack(
    ctx: &mut but_ctx::Context,
    sid: StackId,
    branches: &[String],
    force: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let branches_display = branches.join(", ");

    if !force
        && let Some(mut inout) = out.prepare_for_terminal_input()
        && inout.confirm(
            format!("Are you sure you want to unapply stack with branches '{branches_display}'?"),
            ConfirmDefault::No,
        )? == Confirm::No
    {
        bail!("Aborted unapply operation.");
    }

    // Create snapshot before destructive operation
    create_snapshot(ctx, branches);

    but_api::legacy::virtual_branches::unapply_stack(ctx, sid)?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Unapplied stack with branches '{}' from workspace",
            branches_display
        )?;
    }

    if let Some(out) = out.for_shell() {
        // Shell output: one branch per line
        for branch in branches {
            writeln!(out, "{}", branch)?;
        }
    }

    if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "unapplied": true,
            "branches": branches
        }))?;
    }

    Ok(())
}
