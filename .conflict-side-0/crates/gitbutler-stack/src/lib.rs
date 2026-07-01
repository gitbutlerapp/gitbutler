#![warn(clippy::indexing_slicing)]
mod stack;
mod state;
mod target;

pub use stack::Stack;
#[expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]
pub use state::{VirtualBranches as VirtualBranchesState, VirtualBranchesHandle};
pub use target::Target;

mod heads;
pub use heads::add_head;
pub use stack::PatchReferenceUpdate;

mod stack_branch;
pub use stack::canned_branch_name;
pub use stack_branch::{BranchCommitIds, StackBranch};
