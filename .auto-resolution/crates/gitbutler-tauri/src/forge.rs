use but_api::{commands::forge, IpcContext};
use gitbutler_forge::forge::ForgeName;
use gitbutler_project::ProjectId;
use std::path::PathBuf;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn pr_templates(
    ipc_ctx: State<'_, IpcContext>,
    project_id: ProjectId,
    forge: ForgeName,
) -> Result<Vec<String>, Error> {
    forge::pr_templates(&ipc_ctx, forge::PrTemplatesParams { project_id, forge })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx))]
pub fn pr_template(
    ipc_ctx: State<'_, IpcContext>,
    project_id: ProjectId,
    relative_path: PathBuf,
    forge: ForgeName,
) -> Result<String, Error> {
    forge::pr_template(
        &ipc_ctx,
        forge::PrTemplateParams {
            project_id,
            relative_path,
            forge,
        },
    )
}
