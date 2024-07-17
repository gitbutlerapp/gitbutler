mod branch;
pub use branch::{Branch, BranchCreateRequest, BranchId, BranchUpdateRequest};
mod branch_ext;
pub use branch_ext::BranchExt;
mod dedup;
pub use dedup::{dedup, dedup_fmt};
mod file_ownership;
pub use file_ownership::OwnershipClaim;
mod ownership;
pub use ownership::{reconcile_claims, BranchOwnershipClaims, ClaimOutcome};
pub mod serde;
mod target;
pub use target::Target;

mod state;
pub use state::VirtualBranches as VirtualBranchesState;
pub use state::VirtualBranchesHandle;

use lazy_static::lazy_static;
lazy_static! {
    pub static ref GITBUTLER_INTEGRATION_REFERENCE: gitbutler_reference::LocalRefname =
        gitbutler_reference::LocalRefname::new("gitbutler/integration", None);
}

pub const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME: &str = "GitButler";
pub const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";
