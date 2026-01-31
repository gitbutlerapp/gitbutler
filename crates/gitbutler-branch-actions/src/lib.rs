//! GitButler internal library containing functionality related to branches, i.e. the virtual branches implementation
mod actions;
// This is our API
pub use actions::{
    amend, create_virtual_branch, create_virtual_branch_from_branch, delete_local_branch,
    fetch_from_remotes, get_initial_integration_steps_for_branch, integrate_branch_with_steps,
    integrate_upstream, integrate_upstream_commits, move_branch, move_commit, push_base_branch,
    reorder_stack, resolve_upstream_integration, set_base_branch, set_target_push_remote,
    squash_commits, tear_off_branch, unapply_stack, undo_commit, update_commit_message,
    update_stack_order, upstream_integration_statuses,
};
mod squash;

mod r#virtual;
/// Avoid using these!
/// This was previously `pub use r#virtual::*;`
pub mod internal {
    pub use super::{branch_upstream_integration, r#virtual::*};
}

mod branch_manager;
pub use branch_manager::{BranchManagerExt, CreateBranchFromBranchOutcome};

pub mod base;
pub use base::BaseBranch;

pub mod upstream_integration;

mod integration;
pub use integration::{GITBUTLER_WORKSPACE_COMMIT_TITLE, update_workspace_commit};

mod remote;

pub mod branch_upstream_integration;
mod move_branch;
mod move_commits;
pub mod reorder;
pub use reorder::StackOrder;
mod undo_commit;

mod author;
mod gravatar;
use gitbutler_stack::VirtualBranchesHandle;

trait VirtualBranchesExt {
    fn virtual_branches(&self) -> VirtualBranchesHandle;
}

impl VirtualBranchesExt for but_ctx::Context {
    fn virtual_branches(&self) -> VirtualBranchesHandle {
        VirtualBranchesHandle::new(self.project_data_dir())
    }
}

mod branch;
pub use branch::{
    Author, BranchListing, BranchListingDetails, BranchListingFilter, get_branch_listing_details,
    list_branches,
};
pub use move_branch::MoveBranchResult;
pub use move_commits::MoveCommitIllegalAction;

pub mod hooks;
pub mod stack;
