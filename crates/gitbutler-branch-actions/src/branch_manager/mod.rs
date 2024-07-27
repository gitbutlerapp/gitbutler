use gitbutler_command_context::OpenWorkspaceContext;

mod branch_creation;
mod branch_removal;

pub struct BranchManager<'l> {
    project_repository: &'l OpenWorkspaceContext,
}

pub trait BranchManagerExt {
    fn branch_manager(&self) -> BranchManager;
}

impl BranchManagerExt for OpenWorkspaceContext {
    fn branch_manager(&self) -> BranchManager {
        BranchManager {
            project_repository: self,
        }
    }
}
