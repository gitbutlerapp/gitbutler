use but_ctx::Context;

mod branch_creation;

#[deprecated(
    note = "legacy test helper; use but_workspace::branch::create_reference for application code"
)]
pub struct BranchManager<'l> {
    ctx: &'l Context,
}

#[deprecated(
    note = "legacy test helper; use but_workspace::branch::create_reference for application code"
)]
pub trait BranchManagerExt {
    fn branch_manager(&self) -> BranchManager<'_>;
}

impl BranchManagerExt for Context {
    fn branch_manager(&self) -> BranchManager<'_> {
        BranchManager { ctx: self }
    }
}
