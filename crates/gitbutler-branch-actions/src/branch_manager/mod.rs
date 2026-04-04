pub use branch_creation::CreateBranchFromBranchOutcome;
use but_ctx::Context;

mod branch_creation;
mod branch_removal;

/// Note that this checks out the commit and sets the HEAD ref to point to it.
pub struct BranchManager<'l> {
    ctx: &'l Context,
}

pub trait BranchManagerExt {
    fn branch_manager(&self) -> BranchManager<'_>;
}

impl BranchManagerExt for Context {
    fn branch_manager(&self) -> BranchManager<'_> {
        BranchManager { ctx: self }
    }
}
