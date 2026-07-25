//! GitButler internal library containing functionality related to branches, i.e. the virtual branches implementation
#![expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]

mod actions;
// This is our API
pub use actions::set_base_branch;

mod branch_manager;
pub use branch_manager::BranchManagerExt;

pub mod base;
pub use base::BaseBranch;

mod integration;
pub use integration::{GITBUTLER_WORKSPACE_COMMIT_TITLE, update_workspace_commit};

mod remote;

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
