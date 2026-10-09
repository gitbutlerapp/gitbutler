use std::{fmt::Display, path::PathBuf};

use but_project_handle::ProjectHandleOrLegacyProjectId;
use gix::bstr::BString;

/// The checkout of a repository in which files changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checkout {
    /// The main worktree.
    Main,
    /// The linked worktree of this name, i.e. its directory name under `$GIT_COMMON_DIR/worktrees/`.
    Linked(BString),
}

/// A linked worktree to watch alongside the main worktree.
#[derive(Debug, Clone)]
pub struct LinkedWorktree {
    /// The directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub name: BString,
    /// The checkout directory.
    pub workdir: PathBuf,
}

/// An event for internal use, representing file system changes.
#[derive(Debug)]
pub enum InternalEvent {
    // From file monitor
    /// Paths relative to the git dir of the checkout.
    GitFilesChange(ProjectHandleOrLegacyProjectId, Checkout, Vec<PathBuf>),
    /// Paths relative to the working directory of the checkout.
    ProjectFilesChange(ProjectHandleOrLegacyProjectId, Checkout, Vec<PathBuf>),
}

impl Display for InternalEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternalEvent::GitFilesChange(project_id, checkout, paths) => {
                write!(
                    f,
                    "GitFileChange({}, {:?}, {})",
                    project_id,
                    checkout,
                    comma_separated_paths(paths)
                )
            }
            InternalEvent::ProjectFilesChange(project_id, checkout, paths) => {
                write!(
                    f,
                    "ProjectFileChange({}, {:?}, {})",
                    project_id,
                    checkout,
                    comma_separated_paths(paths)
                )
            }
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
