mod file_ownership;
mod ownership;
mod stack;
mod state;
mod target;

pub use file_ownership::OwnershipClaim;
pub use ownership::{reconcile_claims, BranchOwnershipClaims, ClaimOutcome};
pub use stack::{Stack, StackId};
pub use state::{VirtualBranches as VirtualBranchesState, VirtualBranchesHandle};
pub use target::Target;

mod heads;
mod series;
pub use series::Series;
pub use stack::{commit_by_oid_or_change_id, CommitsForId, PatchReferenceUpdate, TargetUpdate};
