use crate::error::Error;
use gitbutler_project::ProjectId;
use tracing::instrument;

/// The array of unified diffs matches `changes`, so that `result[n] = unified_diff_of(changes[n])`.
#[tauri::command(async)]
#[instrument(skip(projects, change), err(Debug))]
pub fn tree_change_diffs(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    change: but_core::TreeChange,
) -> anyhow::Result<but_core::UnifiedDiff, Error> {
    let project = projects.get(project_id)?;
    let repo = gix::open(project.path).map_err(anyhow::Error::from)?;
    change.unified_diff(&repo).map_err(Into::into)
}
