use std::fmt::Display;
use std::path::PathBuf;

use gitbutler_core::projects::ProjectId;
use gitbutler_core::virtual_branches;

/// An event for internal use, as merge between [super::file_monitor::Event] and [Action].
#[derive(Debug)]
pub(super) enum InternalEvent {
    // From public action API
    CalculateVirtualBranches(ProjectId),

    // From file monitor
    GitFilesChange(ProjectId, Vec<PathBuf>),
    ProjectFilesChange(ProjectId, Vec<PathBuf>),
    // Triggered on change in the `.git/gitbutler` directory
    GitButlerOplogChange(ProjectId),
}

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

impl Display for InternalEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternalEvent::GitFilesChange(project_id, paths) => {
                write!(
                    f,
                    "GitFileChange({}, {})",
                    project_id,
                    comma_separated_paths(paths)
                )
            }
            InternalEvent::GitButlerOplogChange(project_id) => {
                write!(f, "GitButlerOplogChange({})", project_id)
            }
            InternalEvent::ProjectFilesChange(project_id, paths) => {
                write!(
                    f,
                    "ProjectFileChange({}, {})",
                    project_id,
                    comma_separated_paths(paths)
                )
            }
            InternalEvent::CalculateVirtualBranches(pid) => write!(f, "VirtualBranch({})", pid),
        }
    }
}

fn comma_separated_paths(paths: &[PathBuf]) -> String {
    const MAX_LISTING: usize = 5;
    let listing = paths
        .iter()
        .take(MAX_LISTING)
        .filter_map(|path| path.to_str())
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = paths.len().saturating_sub(MAX_LISTING);
    if remaining > 0 {
        format!("{listing} […{remaining} more]")
    } else {
        listing
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
    },
    GitActivity(ProjectId),
    VirtualBranches {
        project_id: ProjectId,
        virtual_branches: virtual_branches::VirtualBranches,
    },
}
