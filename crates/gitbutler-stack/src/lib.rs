mod branch;
mod file_ownership;
mod ownership;
mod state;
mod target;

pub use branch::{Branch, BranchCreateRequest, BranchId, BranchIdentity, BranchUpdateRequest};
pub use file_ownership::OwnershipClaim;
pub use ownership::{reconcile_claims, BranchOwnershipClaims, ClaimOutcome};
pub use state::{VirtualBranches as VirtualBranchesState, VirtualBranchesHandle};
pub use target::Target;
