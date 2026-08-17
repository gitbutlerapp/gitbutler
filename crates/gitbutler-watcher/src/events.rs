use std::{fmt::Display, path::PathBuf, sync::Arc};

use but_ctx::ProjectHandleOrLegacyProjectId;
use gitbutler_operating_modes::OperatingMode;

/// An event for internal use, representing file system changes.
#[derive(Debug)]
pub enum InternalEvent {
    // From file monitor
    GitFilesChange(ProjectHandleOrLegacyProjectId, Vec<PathBuf>),
    ProjectFilesChange(ProjectHandleOrLegacyProjectId, Vec<PathBuf>),
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
            InternalEvent::ProjectFilesChange(project_id, paths) => {
                write!(
                    f,
                    "ProjectFileChange({}, {})",
                    project_id,
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

/// An event telling the receiver something about the state of the application which just changed.
#[derive(Debug, Clone)]
#[expect(missing_docs)]
pub enum Change {
    /// Emitted when a fetch updates the repository's fetched state.
    GitFetch(ProjectHandleOrLegacyProjectId),
    /// Emitted when HEAD changes its symbolic target or becomes attached or detached.
    GitHead {
        project_id: ProjectHandleOrLegacyProjectId,
        /// The symbolic ref HEAD points at, or `None` when HEAD is detached.
        head: Option<String>,
        operating_mode: OperatingMode,
    },
    /// Emitted when the commit at the current HEAD changes.
    GitActivity {
        project_id: ProjectHandleOrLegacyProjectId,
        head_sha: String,
    },
    /// Emitted when branches, remote-tracking state, or an external refresh may have changed which
    /// commits and branches compose the GitButler workspace.
    WorkspaceActivity {
        project_id: ProjectHandleOrLegacyProjectId,
    },
    /// Emitted after worktree files or the index change. Carries freshly computed file diffs
    /// together with hunk assignment and dependency information.
    WorktreeChanges {
        project_id: ProjectHandleOrLegacyProjectId,
        changes: but_hunk_assignment::WorktreeChanges,
        /// The paths of the files that changed.
        ///
        /// This will be empty if the index changed.
        changed_paths: Arc<[PathBuf]>,
    },
}
