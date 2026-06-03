//! GitButler internal library containing functionality related to branches, i.e. the virtual branches implementation
#![expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]

mod actions;
// This is our API
pub use actions::{
    create_virtual_branch, create_virtual_branch_from_branch_with_perm,
    get_initial_integration_steps_for_branch, integrate_branch_with_steps, integrate_upstream,
    integrate_upstream_commits, push_base_branch, resolve_upstream_integration, set_base_branch,
    set_target_push_remote, upstream_integration_statuses,
};

mod r#virtual;

mod branch_manager;
pub use branch_manager::{BranchManagerExt, CreateBranchFromBranchOutcome};

pub mod base;
pub use base::BaseBranch;

pub mod upstream_integration;

mod integration;
pub use integration::{
    GITBUTLER_WORKSPACE_COMMIT_TITLE, update_workspace_commit,
    update_workspace_commit_with_vb_state,
};

mod remote;

pub mod branch_upstream_integration;

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

pub mod hooks;
pub mod stack;
