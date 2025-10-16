//! GitButler internal library containing functionality related to branches, i.e. the virtual branches implementation
mod actions;
// This is our API
pub use actions::{
    amend, can_apply_remote_branch, create_commit, create_virtual_branch,
    create_virtual_branch_from_branch, delete_local_branch, fetch_from_remotes, find_commit,
    find_git_branches, get_initial_integration_steps_for_branch, get_uncommited_files,
    insert_blank_commit, integrate_branch_with_steps, integrate_upstream,
    integrate_upstream_commits, list_commit_files, move_branch, move_commit, push_base_branch,
    reorder_stack, resolve_upstream_integration, set_base_branch, set_target_push_remote,
    squash_commits, tear_off_branch, unapply_stack, undo_commit, update_commit_message,
    update_stack_order, update_virtual_branch, upstream_integration_statuses,
};
mod squash;

mod r#virtual;
pub use r#virtual::{BranchStatus, VirtualBranchHunksByPathMap};
/// Avoid using these!
/// This was previously `pub use r#virtual::*;`
pub mod internal {
    pub use super::branch_upstream_integration;
    pub use super::remote::find_git_branches;
    pub use super::r#virtual::*;
}

mod branch_manager;
pub use branch_manager::{BranchManager, BranchManagerExt, CreateBranchFromBranchOutcome};

pub mod base;
pub use base::BaseBranch;

mod dependencies;
pub use dependencies::compute_workspace_dependencies;

pub mod upstream_integration;

mod integration;
pub use integration::{update_workspace_commit, verify_branch};

mod file;
pub use file::{Get, RemoteBranchFile};

mod remote;
pub use remote::{RemoteBranchData, RemoteCommit};

pub mod branch_upstream_integration;
mod move_branch;
mod move_commits;
pub mod reorder;
pub use reorder::{SeriesOrder, StackOrder};
mod undo_commit;

mod author;
mod gravatar;
mod status;
use gitbutler_stack::VirtualBranchesHandle;
pub use status::get_applied_status;
trait VirtualBranchesExt {
    fn virtual_branches(&self) -> VirtualBranchesHandle;
}

impl VirtualBranchesExt for gitbutler_project::Project {
    fn virtual_branches(&self) -> VirtualBranchesHandle {
        VirtualBranchesHandle::new(self.gb_dir())
    }
}

mod branch;
mod hunk;
pub use hunk::{VirtualBranchHunkRange, VirtualBranchHunkRangeMap};

pub use branch::{
    Author, BranchListing, BranchListingDetails, BranchListingFilter, get_branch_listing_details,
    list_branches,
};

pub use integration::GITBUTLER_WORKSPACE_COMMIT_TITLE;

pub use move_branch::MoveBranchResult;
pub use move_commits::MoveCommitIllegalAction;

pub mod hooks;
pub mod stack;
