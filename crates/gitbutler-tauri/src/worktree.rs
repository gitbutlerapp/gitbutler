use crate::error::Error;
use but_core::IgnoredWorktreeChange;
use gitbutler_project::ProjectId;
use serde::Serialize;
use std::path::PathBuf;
use tracing::instrument;

/// This UI-version of [`but_core::worktree_changes()`] simplifies the `git status` information for display in
/// the user interface as it is right now. From here, it's always possible to add more information as the need arises.
///
/// ### Notable Transformations
/// * There is no notion of an index (`.git/index`) - all changes seem to have happened in the worktree.
/// * Modifications that were made to the index will be ignored *only if* there is a worktree modification to the same file.
/// * conflicts are ignored
///
/// All ignored status changes are also provided so they can be displayed separately.
#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn worktree_changes(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
) -> anyhow::Result<WorktreeChanges, Error> {
    let project = projects.get(project_id)?;
    Ok(worktree_changes_by_worktree_dir(project.path)?)
}

pub fn worktree_changes_by_worktree_dir(worktree_dir: PathBuf) -> anyhow::Result<WorktreeChanges> {
    let repo = gix::open(worktree_dir)?;
    Ok(but_core::worktree_changes(&repo)?.into())
}

/// The type returned by [`but_core::worktree_changes()`].
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeChanges {
    /// Changes that could be committed.
    pub changes: Vec<crate::diff::TreeChange>,
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    pub ignored_changes: Vec<IgnoredWorktreeChange>,
}

impl From<but_core::WorktreeChanges> for WorktreeChanges {
    fn from(
        but_core::WorktreeChanges {
            changes,
            ignored_changes,
        }: but_core::WorktreeChanges,
    ) -> Self {
        WorktreeChanges {
            changes: changes.into_iter().map(Into::into).collect(),
            ignored_changes,
        }
    }
}
