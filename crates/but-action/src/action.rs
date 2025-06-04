use std::{fmt::Debug, str::FromStr};

use chrono::NaiveDateTime;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::OplogExt;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::access::WorktreeWritePermission;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ActionHandler, Outcome};

/// Represents a snapshot of an automatic action taken by a GitButler automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButlerMcpAction {
    /// UUID identifier of the action
    id: Uuid,
    /// The prompt that was used to generate the changes that were made, if applicable
    external_prompt: Option<String>,
    /// A description of the change that was made and why it was made - i.e. the information that can be obtained from the caller.
    pub external_summary: String,
    /// The handler / implementation that performed the action.
    handler: ActionHandler,
    /// An optional prompt that was used by the handler to perform the action, if applicable.
    handler_prompt: Option<String>,
    /// A GitBulter Oplog snapshot ID before the action was performed.
    #[serde(serialize_with = "gitbutler_serde::object_id::serialize")]
    snapshot_before: gix::ObjectId,
    /// A GitBulter Oplog snapshot ID after the action was performed.
    #[serde(serialize_with = "gitbutler_serde::object_id::serialize")]
    snapshot_after: gix::ObjectId,
    /// The outcome of the action, if it was successful.
    response: Option<Outcome>,
    /// An error message if the action failed.
    error: Option<String>,
}

/// Represents a snapshot of an automatic action taken by a GitButler automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButlerRevertAction {
    /// UUID identifier of the action
    id: Uuid,
    /// The snapshot representing the revert
    snapshot: String,
    /// A message describing the revert
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum Action {
    McpAction(ButlerMcpAction),
    RevertAction(ButlerRevertAction),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButlerAction {
    id: Uuid,
    created_at: NaiveDateTime,
    action: Action,
}

impl TryFrom<but_db::FilledButlerAction> for ButlerAction {
    type Error = anyhow::Error;

    fn try_from(value: but_db::FilledButlerAction) -> Result<Self, Self::Error> {
        if let Some(mcp) = value.mcp_action {
            Ok(Self {
                id: Uuid::parse_str(&value.id)?,
                created_at: value.created_at,
                action: Action::McpAction(ButlerMcpAction {
                    id: Uuid::parse_str(&mcp.id)?,
                    external_prompt: mcp.external_prompt,
                    external_summary: mcp.external_summary,
                    handler: mcp
                        .handler
                        .parse()
                        .map_err(|_| anyhow::anyhow!("Invalid Handler value"))?,
                    handler_prompt: mcp.handler_prompt,
                    snapshot_before: gix::ObjectId::from_str(&mcp.snapshot_before)?,
                    snapshot_after: gix::ObjectId::from_str(&mcp.snapshot_after)?,
                    response: mcp
                        .response
                        .as_ref()
                        .and_then(|o| serde_json::from_str(o).ok()),
                    error: mcp.error,
                }),
            })
        } else if let Some(revert) = value.revert_action {
            Ok(Self {
                id: Uuid::parse_str(&value.id)?,
                created_at: value.created_at,
                action: Action::RevertAction(ButlerRevertAction {
                    id: Uuid::parse_str(&revert.id)?,
                    snapshot: revert.snapshot,
                    description: revert.description,
                }),
            })
        } else {
            Err(anyhow::anyhow!("Ahhh"))
        }
    }
}

impl TryFrom<ButlerAction> for but_db::FilledButlerAction {
    type Error = anyhow::Error;

    fn try_from(value: ButlerAction) -> Result<Self, Self::Error> {
        let mut output = Self {
            id: value.id.to_string(),
            created_at: value.created_at,
            mcp_action: None,
            revert_action: None,
        };

        match value.action {
            Action::McpAction(mcp) => {
                let response = mcp
                    .response
                    .as_ref()
                    .and_then(|o| serde_json::to_string(o).ok());
                output.mcp_action = Some(but_db::ButlerMcpAction {
                    id: mcp.id.to_string(),
                    external_prompt: mcp.external_prompt,
                    external_summary: mcp.external_summary,
                    handler: mcp.handler.to_string(),
                    handler_prompt: mcp.handler_prompt,
                    snapshot_before: mcp.snapshot_before.to_string(),
                    snapshot_after: mcp.snapshot_after.to_string(),
                    response,
                    error: mcp.error,
                });
            }
            Action::RevertAction(revert) => {
                output.revert_action = Some(but_db::ButlerRevertAction {
                    id: revert.id.to_string(),
                    snapshot: revert.snapshot,
                    description: revert.description,
                });
            }
        };

        Ok(output)
    }
}

impl ButlerAction {
    pub fn new_mcp(
        handler: ActionHandler,
        external_prompt: Option<String>,
        external_summary: String,
        snapshot_before: gix::ObjectId,
        snapshot_after: gix::ObjectId,
        response: &anyhow::Result<Outcome>,
    ) -> Self {
        let (rsp, error) = if let Err(e) = response {
            (None, Some(e.to_string()))
        } else {
            (response.as_ref().ok(), None)
        };

        Self {
            id: Uuid::new_v4(),
            created_at: chrono::Local::now().naive_local(),
            action: Action::McpAction(ButlerMcpAction {
                id: Uuid::new_v4(),
                handler,
                external_prompt,
                external_summary,
                handler_prompt: None,
                snapshot_before,
                snapshot_after,
                response: rsp.cloned(),
                error,
            }),
        }
    }

    pub fn new_revert(snapshot: gix::ObjectId, description: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            created_at: chrono::Local::now().naive_local(),
            action: Action::RevertAction(ButlerRevertAction {
                id: Uuid::new_v4(),
                snapshot: snapshot.to_string(),
                description: description.to_owned(),
            }),
        }
    }
}

pub(crate) fn persist_action(ctx: &mut CommandContext, action: ButlerAction) -> anyhow::Result<()> {
    ctx.db()?
        .butler_actions()
        .insert(action.try_into()?)
        .map_err(|e| anyhow::anyhow!("Failed to persist action: {}", e))?;
    Ok(())
}

pub fn list_actions(
    ctx: &mut CommandContext,
    page: i64,
    page_size: i64,
) -> anyhow::Result<ActionListing> {
    let (total, actions) = ctx
        .db()?
        .butler_actions()
        .list(page, page_size)
        .map_err(|e| anyhow::anyhow!("Failed to list actions: {}", e))?;

    // Filter out any entries that cannot be converted to ButlerAction
    let actions = actions
        .into_iter()
        .filter_map(|a| TryInto::try_into(a).ok())
        .collect::<Vec<_>>();
    Ok(ActionListing { total, actions })
}

pub fn revert(
    ctx: &mut CommandContext,
    snapshot: gix::ObjectId,
    description: &str,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<()> {
    ctx.restore_snapshot(snapshot.to_git2(), perm)?;

    crate::action::persist_action(
        ctx,
        crate::action::ButlerAction::new_revert(snapshot, description),
    )?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionListing {
    pub total: i64,
    pub actions: Vec<ButlerAction>,
}
