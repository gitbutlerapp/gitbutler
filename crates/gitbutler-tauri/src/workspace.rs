use crate::error::Error;
use gitbutler_command_context::CommandContext;
use gitbutler_id::id::Id;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_serde::BStringForFrontend;
use gitbutler_settings::AppSettingsWithDiskSync;
use gitbutler_stack::Stack;
use serde::Serialize;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn stacks(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> anyhow::Result<Vec<StackEntry>, Error> {
    let project = projects.get(project_id)?;
    let stacks = but_workspace::stacks(&project.gb_dir())?;
    Ok(stacks.into_iter().map(Into::into).collect())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stack_branches(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: String,
) -> anyhow::Result<Vec<but_workspace::Branch>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    but_workspace::stack_branches(stack_id, &ctx).map_err(Into::into)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackEntry {
    id: Id<Stack>,
    branch_names: Vec<BStringForFrontend>,
}

impl From<but_workspace::StackEntry> for StackEntry {
    fn from(but_workspace::StackEntry { id, branch_names }: but_workspace::StackEntry) -> Self {
        StackEntry {
            id,
            branch_names: branch_names.into_iter().map(Into::into).collect(),
        }
    }
}
