mod reference_ext;
pub use reference_ext::ReferenceExtGix;
mod branch;
pub use branch::{BranchIdentity, BranchUpdateRequest};
use lazy_static::lazy_static;
lazy_static! {
    pub static ref GITBUTLER_WORKSPACE_REFERENCE: gitbutler_reference::LocalRefname =
        gitbutler_reference::LocalRefname::new("gitbutler/workspace", None);
}
