//! This crate implements various automations that GitButler can perform.

use but_workspace::{VirtualBranchesTomlMetadata, ui::StackEntry};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

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
#[allow(unused)]
pub fn handle_changes_simple(
    ctx: &mut CommandContext,
    request_ctx: &str,
) -> anyhow::Result<HandleChangesResponse> {
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
            gitbutler_branch_actions::set_base_branch(ctx, &default_target.branch, true);
        }
    }

    let repo = ctx.gix_repo()?;

    let stacks = stacks_creating_if_none(ctx, &vb_state, &repo)?;

    // Get the uncommitted changes in the worktree
    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(ctx.project().path.clone())?;
    // Get any assignments that may have been made, which also includes any hunk locks
    let assignments =
        but_hunk_assignment::assignments(ctx).map_err(|err| serde_error::Error::new(&*err))?;

    // let mut guard = ctx.project().exclusive_worktree_access();
    // let perm = guard.write_permission();

    Ok(HandleChangesResponse {
        updated_branches: vec![],
    })
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
pub struct HandleChangesResponse {
    updated_branches: Vec<UpdatedBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedBranch {
    pub branch_name: String,
    pub commits: Vec<String>,
}
