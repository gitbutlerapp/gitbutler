use but_api::commands::forge;
use gitbutler_forge::forge::ForgeName;
use gitbutler_project::ProjectId;
use std::path::PathBuf;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn pr_templates(project_id: ProjectId, forge: ForgeName) -> Result<Vec<String>, Error> {
    forge::pr_templates(forge::PrTemplatesParams { project_id, forge })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn pr_template(
    project_id: ProjectId,
    relative_path: PathBuf,
    forge: ForgeName,
) -> Result<String, Error> {
    forge::pr_template(forge::PrTemplateParams {
        project_id,
        relative_path,
        forge,
    })
}
