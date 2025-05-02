use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use crate::error::Error;
use gitbutler_project as projects;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn agent_read_directory(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    path: &str,
) -> Result<Vec<String>, Error> {
    let project = projects.get(project_id)?;
    let files = std::fs::read_dir(project.path.join(path)).map_err(|err| anyhow::anyhow!(err))?;
    Ok(files
        .filter_map(|f| f.ok().map(|f| f.file_name().to_string_lossy().to_string()))
        .collect())
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn agent_read_file(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    path: &str,
) -> Result<String, Error> {
    let project = projects.get(project_id)?;
    let content =
        std::fs::read_to_string(project.path.join(path)).map_err(|err| anyhow::anyhow!(err))?;
    Ok(content)
}
