mod branch_ext;
pub use branch_ext::BranchExt;
mod reference_ext;
pub use reference_ext::{ReferenceExt, ReferenceExtGix};
mod dedup;
pub use dedup::{dedup, dedup_fmt};
mod branch;
pub mod serde;
pub use branch::{BranchCreateRequest, BranchIdentity, BranchUpdateRequest};
use lazy_static::lazy_static;
lazy_static! {
    pub static ref GITBUTLER_WORKSPACE_REFERENCE: gitbutler_reference::LocalRefname =
        gitbutler_reference::LocalRefname::new("gitbutler/workspace", None);
}
