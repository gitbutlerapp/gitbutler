use crate::error::Error;
use crate::worktree::TreeChange;
use gitbutler_project::ProjectId;
use gitbutler_serde::BStringForFrontend;
use serde::Serialize;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects, changes), err(Debug))]
pub fn unified_diffs(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<Vec<UnifiedDiff>, Error> {
    let project = projects.get(project_id)?;
    let repo = gix::open(project.path).map_err(anyhow::Error::from)?;

    Ok(changes
        .into_iter()
        .map(|tree_change| tree_change.unified_diff(&repo))
        .collect::<Result<Vec<_>, _>>()?)
}

/// A frontend version of [`but_core::unified_diff::DiffHunk`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub diff: BStringForFrontend,
}

impl From<but_core::unified_diff::DiffHunk> for DiffHunk {
    fn from(
        but_core::unified_diff::DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            diff,
        }: but_core::unified_diff::DiffHunk,
    ) -> Self {
        DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            diff: diff.into(),
        }
    }
}

/// The frontend version of [`but_core::UnifiedDiff`].
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum UnifiedDiff {
    Binary,
    TooLarge {
        #[serde(rename = "sizeInBytes")]
        size_in_bytes: u64,
    },
    Patch {
        hunks: Vec<DiffHunk>,
    },
}

impl From<but_core::UnifiedDiff> for UnifiedDiff {
    fn from(value: but_core::UnifiedDiff) -> Self {
        match value {
            but_core::UnifiedDiff::Binary => UnifiedDiff::Binary,
            but_core::UnifiedDiff::TooLarge { size_in_bytes } => {
                UnifiedDiff::TooLarge { size_in_bytes }
            }
            but_core::UnifiedDiff::Patch { hunks } => UnifiedDiff::Patch {
                hunks: hunks.into_iter().map(Into::into).collect(),
            },
        }
    }
}
