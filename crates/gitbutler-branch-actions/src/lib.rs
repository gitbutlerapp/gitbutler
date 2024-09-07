//! GitButler internal library containing functionality related to branches, i.e. the virtual branches implementation
mod actions;
// This is our API
pub use actions::{
    amend, can_apply_remote_branch, convert_to_real_branch, create_change_reference, create_commit,
    create_virtual_branch, create_virtual_branch_from_branch, delete_local_branch,
    delete_virtual_branch, fetch_from_remotes, get_base_branch_data, get_remote_branch_data,
    get_uncommited_files, get_uncommited_files_reusable, insert_blank_commit,
    integrate_upstream_commits, list_local_branches, list_remote_commit_files,
    list_virtual_branches, list_virtual_branches_cached, move_commit, move_commit_file,
    push_change_reference, push_virtual_branch, reorder_commit, reset_files, reset_virtual_branch,
    set_base_branch, set_target_push_remote, squash, unapply_ownership, undo_commit,
    update_base_branch, update_branch_order, update_change_reference, update_commit_message,
    update_virtual_branch,
};

mod r#virtual;
pub use r#virtual::{BranchStatus, VirtualBranch, VirtualBranchHunksByPathMap, VirtualBranches};
/// Avoid using these!
/// This was previously `pub use r#virtual::*;`
pub mod internal {
    pub use super::r#virtual::*;
    pub use super::remote::list_local_branches;
}

mod branch_manager;
pub use branch_manager::{BranchManager, BranchManagerExt};

mod base;
pub use base::BaseBranch;

mod integration;
pub use integration::{update_workspace_commit, verify_branch};

mod file;
pub use file::{Get, RemoteBranchFile};

mod remote;
pub use remote::{RemoteBranch, RemoteBranchData, RemoteCommit};

pub mod conflicts;

mod author;
mod status;
use gitbutler_branch::VirtualBranchesHandle;
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
mod commit;
mod hunk;

pub use branch::{
    get_branch_listing_details, list_branches, Author, BranchListing, BranchListingDetails,
    BranchListingFilter,
};

pub use integration::GITBUTLER_WORKSPACE_COMMIT_TITLE;
