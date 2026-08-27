use std::{path::PathBuf, sync::Arc};

use but_ctx::ProjectHandleOrLegacyProjectId;
use gitbutler_operating_modes::OperatingMode;

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
        /// True when repository state cached at open time may be stale, so consumers must not
        /// calculate a workspace revision without first opening a fresh repository.
        requires_fresh_repository: bool,
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
