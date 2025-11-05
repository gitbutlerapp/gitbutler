use but_api_macros::api_cmd;
use but_cherry_apply::CherryApplyStatus;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn cherry_apply_status(
    project_id: ProjectId,
    subject: String,
) -> Result<CherryApplyStatus, Error> {
    let project = gitbutler_project::get(project_id)?;
    let guard = project.exclusive_worktree_access();
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let subject_oid = gix::ObjectId::from_hex(subject.as_bytes())
        .map_err(|e| anyhow::anyhow!("Invalid commit ID: {}", e))?;

    but_cherry_apply::cherry_apply_status(&ctx, guard.read_permission(), subject_oid)
        .map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn cherry_apply(project_id: ProjectId, subject: String, target: StackId) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut guard = project.exclusive_worktree_access();
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let subject_oid = gix::ObjectId::from_hex(subject.as_bytes())
        .map_err(|e| anyhow::anyhow!("Invalid commit ID: {}", e))?;

    but_cherry_apply::cherry_apply(&ctx, guard.write_permission(), subject_oid, target)
        .map_err(Into::into)
}
