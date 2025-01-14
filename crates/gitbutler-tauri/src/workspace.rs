use gitbutler_id::id::Id;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_serde::BStringForFrontend;
use gitbutler_stack::Stack;
use serde::Serialize;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn stacks(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> anyhow::Result<Vec<StackEntry>> {
    let project = projects.get(project_id)?;
    let stacks = but_workspace::stacks(&project.gb_dir())?;
    let stacks: Vec<StackEntry> = stacks.into_iter().map(Into::into).collect();
    Ok(stacks)
}

/// Frontend version of [`but_workspace::StackEntry`]
#[derive(Debug, Clone, Serialize)]
pub struct StackEntry {
    pub id: Id<Stack>,
    pub branch_names: Vec<BStringForFrontend>,
}

impl From<but_workspace::StackEntry> for StackEntry {
    fn from(but_workspace::StackEntry { id, branch_names }: but_workspace::StackEntry) -> Self {
        StackEntry {
            id,
            branch_names: branch_names.into_iter().map(Into::into).collect(),
        }
    }
}
