//! Implementation of the `but unapply` command.

use anyhow::{Context as _, bail};
use but_core::ref_metadata::StackId;

use crate::{CliId, IdMap, legacy::workspace::HeadInfoStack, utils::OutputChannel};

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
) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    // Fetch stacks once at the start
    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;

    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
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
                    bail!("Branch '{name}' does not have an associated stack");
                }
            }
            CliId::Commit { .. } => {
                bail!("Cannot unapply a commit. Please specify a branch or stack identifier.");
            }
            CliId::UncommittedHunkOrFile(_)
            | CliId::CommittedFile { .. }
            | CliId::PathPrefix { .. } => {
                bail!("Cannot unapply a file. Please specify a branch or stack identifier.");
            }
            CliId::Uncommitted { .. } => {
                bail!(
                    "Cannot unapply the uncommitted area. Please specify a branch or stack identifier."
                );
            }
        }
    } else {
        bail!(
            "Ambiguous identifier '{}', matches {} items. Please be more specific.",
            identifier,
            parsed_ids.len()
        );
    };

    unapply_stack(ctx, stack_id, &branches, out, guard.write_permission())
}

/// Get branches for a stack by ID, validating the stack exists.
fn get_stack_branches(
    stacks: &[HeadInfoStack],
    stack_id: StackId,
    identifier: &str,
) -> anyhow::Result<(StackId, Vec<String>)> {
    let stack = stacks
        .iter()
        .find(|s| s.id == Some(stack_id))
        .with_context(|| format!("Stack for '{identifier}' not found in workspace"))?;

    let branches: Vec<String> = stack.branch_names().map(ToOwned::to_owned).collect();

    if branches.is_empty() {
        bail!("Stack for '{identifier}' has no branches");
    }

    Ok((stack_id, branches))
}

/// Find a stack by branch name and return the stack ID and branches.
fn find_stack_by_branch_name(
    stacks: &[HeadInfoStack],
    branch_name: &str,
) -> anyhow::Result<(StackId, Vec<String>)> {
    for stack_entry in stacks {
        if stack_entry.contains_branch(branch_name)
            && let Some(sid) = stack_entry.id
        {
            let branches: Vec<String> = stack_entry.branch_names().map(ToOwned::to_owned).collect();
            return Ok((sid, branches));
        }
    }

    bail!("Branch '{branch_name}' not found in any applied stack");
}

fn unapply_stack(
    ctx: &mut but_ctx::Context,
    sid: StackId,
    branches: &[String],
    out: &mut OutputChannel,
    perm: &mut but_core::sync::RepoExclusive,
) -> anyhow::Result<()> {
    let branches_display = branches.join(", ");

    but_api::legacy::virtual_branches::unapply_stack_with_perm(ctx, sid, perm)?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Unapplied stack with branches '{branches_display}' from workspace"
        )?;
    }

    if let Some(out) = out.for_shell() {
        // Shell output: one branch per line
        for branch in branches {
            writeln!(out, "{branch}")?;
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
