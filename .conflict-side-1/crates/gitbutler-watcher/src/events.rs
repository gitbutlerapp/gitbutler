use gitbutler_branch_actions::{RemoteBranchFile, VirtualBranches};
use gitbutler_filemonitor::InternalEvent;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_project::ProjectId;

/// This type captures all operations that can be fed into a watcher that runs in the background.
// TODO(ST): This should not have to be implemented in the Watcher, figure out how this can be moved
//           to application logic at least. However, it's called through a trait in `core`.
#[derive(Debug, PartialEq, Clone)]
#[allow(missing_docs)]
pub enum Action {
    CalculateVirtualBranches(ProjectId),
}

impl Action {
    /// Return the action's associated project id.
    pub fn project_id(&self) -> ProjectId {
        match self {
            Action::CalculateVirtualBranches(project_id) => *project_id,
        }
    }
}

impl From<Action> for InternalEvent {
    fn from(value: Action) -> Self {
        match value {
            Action::CalculateVirtualBranches(v) => InternalEvent::CalculateVirtualBranches(v),
        }
    }
}

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
    UncommitedFiles {
        project_id: ProjectId,
        files: Vec<RemoteBranchFile>,
    },
    WorktreeChanges {
        project_id: ProjectId,
        changes: but_hunk_assignment::WorktreeChanges,
    },
}
