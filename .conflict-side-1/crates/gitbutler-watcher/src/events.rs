use gitbutler_branch_actions::VirtualBranches;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_project::ProjectId;

/// An event telling the receiver something about the state of the application which just changed.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub enum Change {
    GitFetch(ProjectId),
    GitHead {
        project_id: ProjectId,
        head: String,
        operating_mode: OperatingMode,
    },
    GitActivity(ProjectId),
    VirtualBranches {
        project_id: ProjectId,
        virtual_branches: VirtualBranches,
    },
    WorktreeChanges {
        project_id: ProjectId,
        changes: but_hunk_assignment::WorktreeChanges,
    },
}
