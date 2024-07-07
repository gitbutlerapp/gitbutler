pub mod branch;
pub use branch::{Branch, BranchId};
pub mod target;

mod files;
pub use files::*;

pub mod integration;

mod r#virtual;
pub use r#virtual::*;

mod remote;
pub use remote::*;

mod state;
pub use state::VirtualBranches as VirtualBranchesState;
pub use state::VirtualBranchesHandle;

mod author;
pub use author::Author;

use lazy_static::lazy_static;
lazy_static! {
    pub static ref GITBUTLER_INTEGRATION_REFERENCE: crate::git::LocalRefname =
        crate::git::LocalRefname::new("gitbutler/integration", None);
}
