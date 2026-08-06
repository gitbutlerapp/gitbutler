//! GitButler internal library containing functionality related to branches, i.e. the virtual branches implementation

mod actions;
// This is our API
pub use actions::set_base_branch;

pub mod base;
pub use base::BaseBranch;

mod integration;
pub use integration::{
    GITBUTLER_WORKSPACE_COMMIT_TITLE, update_workspace_commit, update_workspace_commit_with_perm,
};

mod remote;

mod gravatar;

mod branch;
pub use branch::{
    Author, BranchListing, BranchListingDetails, BranchListingFilter, get_branch_listing_details,
    list_branches,
};

pub mod stack;
