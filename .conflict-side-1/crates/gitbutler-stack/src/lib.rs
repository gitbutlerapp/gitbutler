#![warn(clippy::indexing_slicing)]
mod file_ownership;
mod ownership;
mod stack;
mod state;
mod target;

pub use file_ownership::OwnershipClaim;
pub use ownership::{BranchOwnershipClaims, ClaimOutcome, reconcile_claims};
pub use stack::{Stack, StackId};
pub use state::{VirtualBranches as VirtualBranchesState, VirtualBranchesHandle};
pub use target::Target;

mod heads;
pub use heads::add_head;
pub use stack::{PatchReferenceUpdate, TargetUpdate};

// This is here because CommitOrChangeId::ChangeId is deprecated, for some reason allow can't be done on the CommitOrChangeId struct
#[expect(deprecated)]
mod stack_branch;
pub use stack::canned_branch_name;
pub use stack_branch::{CommitOrChangeId, StackBranch};
