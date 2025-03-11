#![warn(clippy::indexing_slicing)]
mod file_ownership;
mod ownership;
mod stack;
pub mod stack_context;
mod state;
mod target;

pub use file_ownership::OwnershipClaim;
pub use ownership::{reconcile_claims, BranchOwnershipClaims, ClaimOutcome};
pub use stack::{Stack, StackId};
pub use state::{VirtualBranches as VirtualBranchesState, VirtualBranchesHandle};
pub use target::Target;

mod heads;
pub use heads::add_head;
pub use stack::{commit_by_oid_or_change_id, CommitsForId, PatchReferenceUpdate, TargetUpdate};

// This is here because CommitOrChangeId::ChangeId is deprecated, for some reason allow cant be done on the CommitOrChangeId struct
#[allow(deprecated)]
mod stack_branch;
pub use stack_branch::{CommitOrChangeId, StackBranch};
