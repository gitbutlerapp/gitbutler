#![warn(clippy::indexing_slicing)]
mod stack;
mod state;
mod target;

pub use stack::Stack;
#[expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]
pub use state::VirtualBranchesHandle;
mod stack_branch;
pub use stack_branch::StackBranch;
