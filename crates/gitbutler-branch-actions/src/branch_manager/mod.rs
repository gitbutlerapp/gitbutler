use gitbutler_command_context::CommandContext;

mod branch_creation;
mod branch_removal;

pub struct BranchManager<'l> {
    ctx: &'l CommandContext,
}

pub trait BranchManagerExt {
    fn branch_manager(&self) -> BranchManager;
}

impl BranchManagerExt for CommandContext {
    fn branch_manager(&self) -> BranchManager<'_> {
        BranchManager { ctx: self }
    }
}
