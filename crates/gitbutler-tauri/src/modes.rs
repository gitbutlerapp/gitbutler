use gitbutler_operating_modes::OperatingMode;
use gitbutler_project::Controller;
use gitbutler_project::ProjectId;
use tauri::State;

use crate::error::Error;

#[tauri::command(async)]
pub fn operating_mode(
    projects: State<'_, Controller>,
    project_id: ProjectId,
) -> Result<OperatingMode, Error> {
    let project = projects.get(project_id)?;
    gitbutler_operating_modes::commands::operating_mode(&project).map_err(Into::into)
}
