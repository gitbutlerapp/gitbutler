use crate::error::Error;
use crate::from_json::HexHash;
use but_core::ui::{TreeChange, WorktreeChanges};
use gitbutler_project::ProjectId;
use tracing::instrument;

pub(crate) const UNIDIFF_CONTEXT_LINES: u32 = 3;

/// Provide a unified diff for `change`, but fail if `change` is a [type-change](but_core::ModeFlags::TypeChange)
/// or if it involves a change to a [submodule](gix::object::Kind::Commit).
#[tauri::command(async)]
#[instrument(skip(projects, change), err(Debug))]
pub fn tree_change_diffs(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    change: TreeChange,
) -> anyhow::Result<but_core::UnifiedDiff, Error> {
    let change: but_core::TreeChange = change.into();
    let project = projects.get(project_id)?;
    let repo = gix::open(project.path).map_err(anyhow::Error::from)?;
    change
        .unified_diff(&repo, UNIDIFF_CONTEXT_LINES)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn commit_changes(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    old_commit_id: Option<HexHash>,
    new_commit_id: HexHash,
) -> anyhow::Result<Vec<TreeChange>, Error> {
    let project = projects.get(project_id)?;
    but_core::diff::ui::commit_changes_by_worktree_dir(
        project.path,
        old_commit_id.map(Into::into),
        new_commit_id.into(),
    )
    .map_err(Into::into)
}

/// This UI-version of [`but_core::diff::worktree_changes()`] simplifies the `git status` information for display in
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
    Ok(but_core::diff::ui::worktree_changes_by_worktree_dir(
        project.path,
    )?)
}
