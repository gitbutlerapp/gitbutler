use crate::RequestContext;
use anyhow::Context;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::OplogExt;
use gitbutler_oplog::entry::OperationKind;
use gitbutler_project::ProjectId;
use serde::{Deserialize, Deserializer};
use std::{ops::Deref, str::FromStr};

/// A type that deserializes a hexadecimal hash into an object id automatically.
#[derive(Debug, Clone, Copy)]
struct HexHash(gix::ObjectId);

impl From<HexHash> for gix::ObjectId {
    fn from(value: HexHash) -> Self {
        value.0
    }
}

impl Deref for HexHash {
    type Target = gix::ObjectId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for HexHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        gix::ObjectId::from_str(&hex)
            .map(HexHash)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListSnapshotsParams {
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
    exclude_kind: Option<Vec<OperationKind>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreSnapshotParams {
    project_id: ProjectId,
    sha: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotDiffParams {
    project_id: ProjectId,
    sha: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OplogDiffWorktreesParams {
    project_id: ProjectId,
    before: HexHash,
    after: HexHash,
}

pub fn list_snapshots(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ListSnapshotsParams = serde_json::from_value(params)?;
    let project = ctx
        .project_controller
        .get(params.project_id)
        .context("failed to get project")?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let snapshots = cmd_ctx.list_snapshots(
        params.limit,
        params
            .sha
            .map(|hex| hex.parse().map_err(anyhow::Error::from))
            .transpose()?,
        params.exclude_kind.unwrap_or_default(),
    )?;
    Ok(serde_json::to_value(snapshots)?)
}

pub fn restore_snapshot(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: RestoreSnapshotParams = serde_json::from_value(params)?;
    let project = ctx
        .project_controller
        .get(params.project_id)
        .context("failed to get project")?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();
    cmd_ctx.restore_snapshot(
        params.sha.parse().map_err(anyhow::Error::from)?,
        guard.write_permission(),
    )?;
    Ok(serde_json::Value::Null)
}

pub fn snapshot_diff(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: SnapshotDiffParams = serde_json::from_value(params)?;
    let project = ctx
        .project_controller
        .get(params.project_id)
        .context("failed to get project")?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let diff = cmd_ctx.snapshot_diff(params.sha.parse().map_err(anyhow::Error::from)?)?;
    let diff: Vec<but_core::ui::TreeChange> = diff.into_iter().map(Into::into).collect();
    Ok(serde_json::to_value(diff)?)
}

pub fn oplog_diff_worktrees(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: OplogDiffWorktreesParams = serde_json::from_value(params)?;
    let project = ctx
        .project_controller
        .get(params.project_id)
        .context("failed to get project")?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let before = cmd_ctx.snapshot_workspace_tree(*params.before)?;
    let after = cmd_ctx.snapshot_workspace_tree(*params.after)?;

    let diff = but_core::diff::ui::changes_in_range(cmd_ctx.project().path.clone(), after, before)?;
    Ok(serde_json::to_value(diff)?)
}
