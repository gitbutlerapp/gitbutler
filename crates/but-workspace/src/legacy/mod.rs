use but_oxidize::OidExt;
use gitbutler_command_context::CommandContext;
use gitbutler_stack::VirtualBranchesHandle;
use std::path::Path;

pub mod commit_engine;
pub mod head;
mod integrated;
mod stacks;
pub use head::{
    merge_worktree_with_workspace, remerged_workspace_commit_v2, remerged_workspace_tree_v2,
};

pub mod tree_manipulation;
pub use tree_manipulation::{
    MoveChangesResult,
    move_between_commits::move_changes_between_commits,
    remove_changes_from_commit_in_stack::remove_changes_from_commit_in_stack,
    split_branch::{split_branch, split_into_dependent_branch},
    split_commit::{CommitFiles, CommmitSplitOutcome, split_commit},
};

// TODO: _v3 versions are specifically for the UI, so import them into `ui` instead.
pub use stacks::{
    local_and_remote_commits, stack_branches, stack_details, stack_details_v3, stack_heads_info,
    stacks, stacks_v3,
};

/// Various types for the frontend.
pub mod ui;

mod branch_details;
pub use branch_details::branch_details;

/// High level Stack functions that use primitives from this crate (`but-workspace`)
pub mod stack_ext;

/// Returns the last-seen fork-point that the workspace has with the target branch with which it wants to integrate.
// TODO: at some point this should be optional, integration branch doesn't have to be defined.
pub fn common_merge_base_with_target_branch(gb_dir: &Path) -> anyhow::Result<gix::ObjectId> {
    Ok(VirtualBranchesHandle::new(gb_dir)
        .get_default_target()?
        .sha
        .to_gix())
}

/// Return a list of commits on the target branch
/// Starts either from the target branch or from the provided commit id, up to the limit provided.
///
/// Returns the commits in reverse order, i.e., from the most recent to the oldest.
/// The `Commit` type is the same as that of the other workspace endpoints - for that reason,
/// the fields `has_conflicts` and `state` are somewhat meaningless.
pub fn log_target_first_parent(
    ctx: &CommandContext,
    last_commit_id: Option<gix::ObjectId>,
    limit: usize,
) -> anyhow::Result<Vec<crate::ui::Commit>> {
    let repo = ctx.gix_repo()?;
    let traversal_root_id = match last_commit_id {
        Some(id) => {
            let commit = repo.find_commit(id)?;
            commit.parent_ids().next()
        }
        None => {
            let state = state_handle(&ctx.project().gb_dir());
            let default_target = state.get_default_target()?;
            Some(
                repo.find_reference(&default_target.branch.to_string())?
                    .peel_to_commit()?
                    .id(),
            )
        }
    };
    let traversal_root_id = match traversal_root_id {
        Some(id) => id,
        None => return Ok(vec![]),
    };

    let mut commits: Vec<crate::ui::Commit> = vec![];
    for commit_info in traversal_root_id.ancestors().first_parent_only().all()? {
        if commits.len() == limit {
            break;
        }
        let commit = commit_info?.id().object()?.into_commit();

        commits.push(commit.try_into()?);
    }
    Ok(commits)
}

fn state_handle(gb_state_path: &Path) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(gb_state_path)
}
