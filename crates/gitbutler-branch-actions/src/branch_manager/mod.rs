use gitbutler_command_context::ProjectRepository;

pub mod branch_creation;
pub mod branch_removal;

pub struct BranchManager<'l> {
    project_repository: &'l ProjectRepository,
}

pub trait BranchManagerAccess {
    fn branch_manager(&self) -> BranchManager;
}

impl BranchManagerAccess for ProjectRepository {
    fn branch_manager(&self) -> BranchManager {
        BranchManager {
            project_repository: self,
        }
    }
}
