//! GitButler internal library containing functionaliry related to branches, i.e. the virtual branches implementation
pub mod actions;
pub use actions::VirtualBranchActions;

pub mod r#virtual;
pub use r#virtual::*;

pub mod branch_manager;

pub mod base;

pub mod integration;

pub mod files;

pub mod remote;

pub mod conflicts;

mod author;

use gitbutler_branch::VirtualBranchesHandle;
trait VirtualBranchesExt {
    fn virtual_branches(&self) -> VirtualBranchesHandle;
}

impl VirtualBranchesExt for gitbutler_project::Project {
    fn virtual_branches(&self) -> VirtualBranchesHandle {
        VirtualBranchesHandle::new(self.gb_dir())
    }
}
