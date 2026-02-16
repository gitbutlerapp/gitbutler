#![warn(clippy::indexing_slicing)]
mod stack;
mod state;
mod target;

pub use stack::{Stack, StackId};
pub use state::{VirtualBranches as VirtualBranchesState, VirtualBranchesHandle};
pub use target::Target;

mod heads;
pub use heads::add_head;
pub use stack::PatchReferenceUpdate;

#[expect(deprecated)]
mod stack_branch;
pub use stack::canned_branch_name;
pub use stack_branch::StackBranch;
