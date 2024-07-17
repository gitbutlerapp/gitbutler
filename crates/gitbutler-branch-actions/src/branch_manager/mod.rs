use gitbutler_command_context::ProjectRepository;

mod branch_creation;
mod branch_removal;

pub struct BranchManager<'l> {
    project_repository: &'l ProjectRepository,
}

pub trait BranchManagerExt {
    fn branch_manager(&self) -> BranchManager;
}

impl BranchManagerExt for ProjectRepository {
    fn branch_manager(&self) -> BranchManager {
        BranchManager {
            project_repository: self,
        }
    }
}
