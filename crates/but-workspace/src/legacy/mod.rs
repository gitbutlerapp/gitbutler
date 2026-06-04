#![expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]

use std::path::Path;

use but_ctx::Context;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

pub mod head;
mod integrated;
mod stacks;
pub use head::{
    merge_worktree_with_workspace, remerged_workspace_commit_v2, remerged_workspace_tree_v2,
};

// TODO: _v3 versions are specifically for the UI, so import them into `ui` instead.
#[expect(deprecated, reason = "re-exports stacks_v3 and stack_details_v3")]
pub use stacks::{
    local_and_remote_commits, stack_branches, stack_details_v3, stack_heads_info, stacks_v3,
};

/// Various types for the frontend.
pub mod ui;

/// High level Stack functions that use primitives from this crate (`but-workspace`)
pub mod stack_ext;

pub mod push;
pub use push::workspace_branch_and_ancestors_push;

/// Return a list of commits on the target branch
/// Starts either from the target branch or from the provided commit id, up to the limit provided.
///
/// Returns the commits in reverse order, i.e., from the most recent to the oldest.
/// The `Commit` type is the same as that of the other workspace endpoints - for that reason,
/// the fields `has_conflicts` and `state` are somewhat meaningless.
pub fn log_target_first_parent(
    ctx: &Context,
    last_commit_id: Option<gix::ObjectId>,
    limit: usize,
) -> anyhow::Result<Vec<crate::ui::Commit>> {
    let repo = ctx.repo.get()?;
    let traversal_root_id = match last_commit_id {
        Some(id) => {
            let commit = repo.find_commit(id)?;
            commit.parent_ids().next()
        }
        None => {
            let default_target = ctx.persisted_default_target()?;
            let target_ref_name: gix::refs::FullName =
                default_target.branch.to_string().try_into()?;
            Some(
                repo.find_reference(target_ref_name.as_ref())?
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
        // In shallow repositories, the traversal may hit a commit whose parent
        // objects are not present locally. Stop rather than propagating the error.
        let info = match commit_info {
            Ok(info) => info,
            Err(_) => break,
        };
        let commit = match info.id().object() {
            Ok(obj) => obj.into_commit(),
            Err(_) => break,
        };
        commits.push(commit.try_into()?);
    }
    Ok(commits)
}
/// A filter for the list of stacks.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum StacksFilter {
    /// Show all stacks
    All,
    /// Show only applied stacks
    #[default]
    InWorkspace,
    /// Show only unapplied stacks
    // TODO: figure out where this is used. V2 maybe? If so, it can be removed eventually.
    Unapplied,
}

fn state_handle(gb_state_path: &Path) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(gb_state_path)
}
