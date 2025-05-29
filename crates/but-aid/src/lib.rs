//! This crate implements various automations that GitButler can perform.

use std::collections::HashMap;

mod state;
use anyhow::anyhow;
use but_workspace::{DiffSpec, StackId, VirtualBranchesTomlMetadata, ui::StackEntry};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_oxidize::OidExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

pub use state::list_past_actions;

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
///
/// TODO:
/// - Handle the case of target branch not being configured
/// - Ability to list past actions and their outcomes
pub fn handle_changes_simple(
    ctx: &mut CommandContext,
    change_description: &str,
) -> anyhow::Result<Outcome> {
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();

    let snapshot_before = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::AutoHandleChangesBefore),
            perm,
        )?
        .to_gix();

    let response = handle_changes_simple_inner(ctx, change_description, perm);

    let snapshot_after = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::AutoHandleChangesAfter),
            perm,
        )?
        .to_gix();

    // Add a checkpoint entry
    state::persist_action(
        ctx,
        state::ButlerAction::new(
            state::AutoHandler::HandleChangesSimple,
            change_description.to_owned(),
            snapshot_before,
            snapshot_after,
            &response,
        ),
    )?;

    response
    // Ok(response)
}

fn handle_changes_simple_inner(
    ctx: &mut CommandContext,
    change_description: &str,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<Outcome> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
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
            gitbutler_branch_actions::set_base_branch(ctx, &default_target.branch, true)?;
        }
    }

    let repo = ctx.gix_repo()?;

    // Get any assignments that may have been made, which also includes any hunk locks. Assignments should be updated according to locks where applicable.
    let assignments = but_hunk_assignment::assignments(ctx, true)
        .map_err(|err| serde_error::Error::new(&*err))?;
    if assignments.is_empty() {
        return Ok(Outcome {
            updated_branches: vec![],
        });
    }

    // Get the current stacks in the workspace, creating one if none exists.
    let stacks = stacks_creating_if_none(ctx, &vb_state, &repo)?;

    // Put the assignments into buckets by stack ID.
    let mut stack_assignments: HashMap<StackId, Vec<DiffSpec>> =
        stacks.iter().map(|s| (s.id, vec![])).collect();
    let default_stack_id = stacks
        .first()
        .map(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("No stacks found in the workspace"))?;
    for assignment in assignments {
        if let Some(stack_id) = assignment.stack_id {
            stack_assignments
                .entry(stack_id)
                .or_default()
                .push(assignment.into());
        } else {
            stack_assignments
                .entry(default_stack_id)
                .or_default()
                .push(assignment.into());
        }
    }
    // Go over the stack_assignments and flatten the diff specs for each stack.
    for (_, specs) in stack_assignments.iter_mut() {
        *specs = flatten_diff_specs(specs.clone());
    }

    let mut updated_branches = vec![];

    for (stack_id, diff_specs) in stack_assignments {
        if diff_specs.is_empty() {
            continue;
        }

        let stack_branch_name = stacks
            .iter()
            .find(|s| s.id == stack_id)
            .and_then(|s| s.heads.first().map(|h| h.name.to_string()))
            .ok_or(anyhow!("Could not find associated reference name"))?;

        let outcome = but_workspace::commit_engine::create_commit_simple(
            ctx,
            stack_id,
            None,
            diff_specs,
            change_description.to_owned(),
            stack_branch_name.clone(),
            perm,
        )?;

        if let Some(new_commit) = outcome.new_commit {
            updated_branches.push(UpdatedBranch {
                branch_name: stack_branch_name,
                new_commits: vec![new_commit.to_string()],
            });
        }
    }

    Ok(Outcome { updated_branches })
}

/// If there are multiple diffs spces where path and previous_path are the same, collapse them into one.
fn flatten_diff_specs(input: Vec<DiffSpec>) -> Vec<DiffSpec> {
    let mut output: HashMap<String, DiffSpec> = HashMap::new();
    for spec in input {
        let key = format!(
            "{}:{}",
            spec.path,
            spec.previous_path
                .clone()
                .map(|p| p.to_string())
                .unwrap_or_default()
        );
        output
            .entry(key)
            .and_modify(|e| e.hunk_headers.extend(spec.hunk_headers.clone()))
            .or_insert(spec);
    }
    output.into_values().collect()
}

/// Returns the currently applied stacks, creating one if none exists.
fn stacks_creating_if_none(
    ctx: &CommandContext,
    vb_state: &VirtualBranchesHandle,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<StackEntry>> {
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    let stacks = but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace)?;
    if stacks.is_empty() {
        let template = gitbutler_stack::canned_branch_name(ctx.repo())?;
        let branch_name = gitbutler_stack::Stack::next_available_name(
            &ctx.gix_repo()?,
            vb_state,
            template,
            false,
        )?;
        let create_req = BranchCreateRequest {
            name: Some(branch_name),
            ownership: None,
            order: None,
            selected_for_changes: None,
        };
        gitbutler_branch_actions::create_virtual_branch(ctx, &create_req)?;
        let stacks =
            but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace)?;
        if stacks.is_empty() {
            anyhow::bail!("No stacks found in the workspace after creation")
        }
        Ok(stacks)
    } else {
        Ok(stacks)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    updated_branches: Vec<UpdatedBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedBranch {
    pub branch_name: String,
    pub new_commits: Vec<String>,
}
