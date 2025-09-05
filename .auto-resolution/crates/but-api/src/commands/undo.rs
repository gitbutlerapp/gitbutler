use anyhow::Context;
use but_api_macros::api_cmd;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::entry::OperationKind;
use gitbutler_oplog::{OplogExt, entry::Snapshot};
use gitbutler_project::ProjectId;

use crate::error::Error;

#[api_cmd]
pub fn list_snapshots(
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
    exclude_kind: Option<Vec<OperationKind>>,
) -> Result<Vec<Snapshot>, Error> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let snapshots = ctx.list_snapshots(
        limit,
        sha.map(|hex| hex.parse().map_err(anyhow::Error::from))
            .transpose()?,
        exclude_kind.unwrap_or_default(),
    )?;
    Ok(snapshots)
}

#[api_cmd]
pub fn restore_snapshot(project_id: ProjectId, sha: String) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();
    ctx.restore_snapshot(
        sha.parse().map_err(anyhow::Error::from)?,
        guard.write_permission(),
    )?;
    Ok(())
}

#[api_cmd]
pub fn snapshot_diff(
    project_id: ProjectId,
    sha: String,
) -> Result<Vec<but_core::ui::TreeChange>, Error> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let diff = ctx.snapshot_diff(sha.parse().map_err(anyhow::Error::from)?)?;
    let diff: Vec<but_core::ui::TreeChange> = diff.into_iter().map(Into::into).collect();
    Ok(diff)
}
