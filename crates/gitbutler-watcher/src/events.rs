use std::fmt::Display;
use std::path::PathBuf;

use gitbutler_core::{deltas, reader, sessions::SessionId, virtual_branches};
use gitbutler_core::{projects::ProjectId, sessions};

/// An event for internal use, as merge between [super::file_monitor::Event] and [Action].
#[derive(Debug)]
pub(super) enum InternalEvent {
    // From public action API
    Flush(ProjectId, sessions::Session),
    CalculateVirtualBranches(ProjectId),

    // From file monitor
    GitFilesChange(ProjectId, Vec<PathBuf>),
    ProjectFilesChange(ProjectId, Vec<PathBuf>),
}

/// This type captures all operations that can be fed into a watcher that runs in the background.
// TODO(ST): This should not have to be implemented in the Watcher, figure out how this can be moved
//           to application logic at least. However, it's called through a trait in `core`.
#[derive(Debug, PartialEq, Clone)]
#[allow(missing_docs)]
pub enum Action {
    Flush(ProjectId, sessions::Session),
    CalculateVirtualBranches(ProjectId),
}

impl Action {
    /// Return the action's associated project id.
    pub fn project_id(&self) -> ProjectId {
        match self {
            Action::Flush(project_id, _) | Action::CalculateVirtualBranches(project_id) => {
                *project_id
            }
        }
    }
}

impl From<Action> for InternalEvent {
    fn from(value: Action) -> Self {
        match value {
            Action::Flush(a, b) => InternalEvent::Flush(a, b),
            Action::CalculateVirtualBranches(v) => InternalEvent::CalculateVirtualBranches(v),
        }
    }
}

impl Display for InternalEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternalEvent::Flush(project_id, session) => {
                write!(f, "Flush({}, {})", project_id, session.id)
            }
            InternalEvent::GitFilesChange(project_id, paths) => {
                write!(
                    f,
                    "GitFileChange({}, {})",
                    project_id,
                    comma_separated_paths(paths)
                )
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
    GitIndex(ProjectId),
    GitFetch(ProjectId),
    GitHead {
        project_id: ProjectId,
        head: String,
    },
    GitActivity(ProjectId),
    File {
        project_id: ProjectId,
        session_id: SessionId,
        file_path: PathBuf,
        contents: Option<reader::Content>,
    },
    Session {
        project_id: ProjectId,
        session: sessions::Session,
    },
    Deltas {
        project_id: ProjectId,
        session_id: SessionId,
        deltas: Vec<deltas::Delta>,
        relative_file_path: PathBuf,
    },
    VirtualBranches {
        project_id: ProjectId,
        virtual_branches: virtual_branches::VirtualBranches,
    },
}
