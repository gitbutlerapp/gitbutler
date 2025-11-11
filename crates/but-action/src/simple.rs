use std::collections::HashMap;

use crate::{Outcome, Source, default_target_setting_if_none};
use anyhow::anyhow;
use but_core::DiffSpec;
use but_core::ref_metadata::StackId;
use but_oxidize::OidExt;
use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::VirtualBranchesHandle;
use uuid::Uuid;

/// This is a GitButler automation which allows easy handling of uncommitted changes in a repository.
/// At a high level, it will:
///   - Checkout GitButler's workspace branch if not already checked out
///   - Create a new branch if necessary (using a generic canned branch name)
///   - Create a new commit with all uncommitted changes found in the worktree (the request context is used as the commit message)
///
/// Avery time this automation is ran, GitButler will aslo:
///   - Create an oplog snaposhot entry _before_ the automation is executed
///   - Create an oplog snapshot entry _after_ the automation is executed
///   - Create a separate persisted entry recording the request context and IDs for the two oplog snapshots
pub fn handle_changes(
    ctx: &mut CommandContext,
    change_summary: &str,
    external_prompt: Option<String>,
    source: Source,
    exclusive_stack: Option<StackId>,
) -> anyhow::Result<(Uuid, Outcome)> {
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();

    let vb_state = &VirtualBranchesHandle::new(ctx.project().gb_dir());
    default_target_setting_if_none(ctx, vb_state)?; // Create a default target if none exists.

    let snapshot_before = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::AutoHandleChangesBefore),
            perm,
        )?
        .to_gix();

    let response = handle_changes_simple_inner(
        ctx,
        change_summary,
        external_prompt.clone(),
        vb_state,
        perm,
        exclusive_stack,
    );

    let snapshot_after = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::AutoHandleChangesAfter),
            perm,
        )?
        .to_gix();

    let action = crate::action::ButlerAction::new(
        crate::ActionHandler::HandleChangesSimple,
        external_prompt,
        change_summary.to_owned(),
        snapshot_before,
        snapshot_after,
        &response,
        source,
    );
    let response = response.map(|outcome| (action.id, outcome));
    crate::action::persist_action(ctx, action)?;
    response
}

fn handle_changes_simple_inner(
    ctx: &mut CommandContext,
    change_summary: &str,
    external_prompt: Option<String>,
    vb_state: &VirtualBranchesHandle,
    perm: &mut WorktreeWritePermission,
    exclusive_stack: Option<StackId>,
) -> anyhow::Result<Outcome> {
    match gitbutler_operating_modes::operating_mode(ctx) {
        OperatingMode::OpenWorkspace => {
            // No action needed, we're already in the workspace
        }
        OperatingMode::Edit(_) => {
            return Err(anyhow::anyhow!(
                "Cannot handle changes while in edit mode. Please exit edit mode first."
            ));
        }
        OperatingMode::OutsideWorkspace(_) => {
            let default_target = vb_state.get_default_target()?;
            gitbutler_branch_actions::set_base_branch(ctx, &default_target.branch, perm)?;
        }
    }

    let repo = ctx.gix_repo()?;

    // Get any assignments that may have been made, which also includes any hunk locks. Assignments should be updated according to locks where applicable.
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        true,
        None::<Vec<but_core::TreeChange>>,
        None,
    )
    .map_err(|err| serde_error::Error::new(&*err))?;
    if assignments.is_empty() {
        return Ok(Outcome {
            updated_branches: vec![],
        });
    }

    // Get the current stacks in the workspace, creating one if none exists.
    let stacks = crate::stacks_creating_if_none(ctx, vb_state, &repo, perm)?;

    // Put the assignments into buckets by stack ID.
    let mut stack_assignments: HashMap<StackId, Vec<DiffSpec>> = stacks
        .iter()
        .filter_map(|s| Some((s.id?, vec![])))
        .collect();
    let default_stack_id = stacks
        .first()
        .and_then(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("No stacks found in the workspace"))?;
    for assignment in assignments {
        if let Some(stack_id) = assignment.stack_id {
            stack_assignments
                .entry(stack_id)
                .or_default()
                .push(assignment.into());
        } else if exclusive_stack.is_none() {
            // If there is an exclusive stack. We don't want to do anything with
            // the unassigned changes.
            stack_assignments
                .entry(default_stack_id)
                .or_default()
                .push(assignment.into());
        }
    }
    // Go over the stack_assignments and flatten the diff specs for each stack.
    for (_, specs) in stack_assignments.iter_mut() {
        *specs = but_workspace::flatten_diff_specs(specs.clone());
    }

    let mut updated_branches = vec![];

    let commit_message = if let Some(prompt) = external_prompt {
        format!("{prompt}\n\n{change_summary}")
    } else {
        change_summary.to_string()
    };

    for (stack_id, diff_specs) in stack_assignments {
        if diff_specs.is_empty() {
            continue;
        }
        if let Some(exclusive_stack) = exclusive_stack
            && exclusive_stack != stack_id
        {
            continue; // Skip stacks that are not the exclusive stack.
        }

        let stack_branch_name = stacks
            .iter()
            .find(|s| s.id == Some(stack_id))
            .and_then(|s| s.heads.first().map(|h| h.name.to_string()))
            .ok_or(anyhow!("Could not find associated reference name"))?;

        let outcome = but_workspace::legacy::commit_engine::create_commit_simple(
            ctx,
            stack_id,
            None,
            diff_specs,
            commit_message.clone(),
            stack_branch_name.clone(),
            perm,
        )?;

        if let Some(new_commit) = outcome.new_commit {
            updated_branches.push(crate::UpdatedBranch {
                stack_id,
                branch_name: stack_branch_name,
                new_commits: vec![new_commit.to_string()],
            });
        }
    }

    Ok(Outcome { updated_branches })
}
