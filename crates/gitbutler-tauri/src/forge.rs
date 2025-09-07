use but_api::commands::forge;
use gitbutler_forge::forge::ForgeName;
use gitbutler_project::ProjectId;
use std::path::PathBuf;

use but_api::error::Error;

#[tauri::command(async)]
pub fn pr_templates(project_id: ProjectId, forge: ForgeName) -> Result<Vec<String>, Error> {
    forge::pr_templates(project_id, forge)
}

#[tauri::command(async)]
pub fn pr_template(
    project_id: ProjectId,
    relative_path: PathBuf,
    forge: ForgeName,
) -> Result<String, Error> {
    forge::pr_template(project_id, relative_path, forge)
}
