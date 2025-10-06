use crate::error::Error;
use but_api_macros::api_cmd;
use but_cherry_apply::{CherryApplyStatus, cherry_apply_status};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::OidExt;
use gitbutler_project::{Project, ProjectId};
use tracing::instrument;

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn cherry_apply_status(project_id: ProjectId, subject: String) -> Result<CherryApplyStatus, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let subject_oid = gix::ObjectId::from_hex(subject.as_bytes())
        .map_err(|e| anyhow::anyhow!("Invalid commit ID: {}", e))?;

    but_cherry_apply::cherry_apply_status(&ctx, subject_oid).map_err(Into::into)
}
