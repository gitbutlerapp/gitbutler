use crate::error::Error;
use gitbutler_project::ProjectId;
use std::path::PathBuf;
use tracing::instrument;

/// This UI-version of [`but_core::worktree::changes()`] simplifies the `git status` information for display in
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
) -> anyhow::Result<but_core::WorktreeChanges, Error> {
    let project = projects.get(project_id)?;
    Ok(changes_in_worktree(project.path)?)
}

fn changes_in_worktree(worktree_dir: PathBuf) -> anyhow::Result<but_core::WorktreeChanges> {
    let repo = gix::open(worktree_dir).map_err(anyhow::Error::new)?;
    but_core::worktree::changes(&repo)
}
