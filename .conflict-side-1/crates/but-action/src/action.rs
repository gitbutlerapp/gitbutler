use std::{fmt::Debug, str::FromStr};

use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ActionHandler, Outcome};

/// Represents a snapshot of an automatic action taken by a GitButler automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButlerAction {
    /// UUID identifier of the action
    id: Uuid,
    /// The time when the action was performed.
    created_at: chrono::NaiveDateTime,
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

impl TryFrom<but_db::ButlerAction> for ButlerAction {
    type Error = anyhow::Error;

    fn try_from(value: but_db::ButlerAction) -> Result<Self, Self::Error> {
        let response = value
            .response
            .as_ref()
            .and_then(|o| serde_json::from_str(o).ok());
        Ok(Self {
            id: Uuid::parse_str(&value.id)?,
            created_at: value.created_at,
            external_prompt: value.external_prompt,
            external_summary: value.external_summary,
            handler: value
                .handler
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid Handler value"))?,
            handler_prompt: value.handler_prompt,
            snapshot_before: gix::ObjectId::from_str(&value.snapshot_before)?,
            snapshot_after: gix::ObjectId::from_str(&value.snapshot_after)?,
            response,
            error: value.error,
        })
    }
}

impl TryFrom<ButlerAction> for but_db::ButlerAction {
    type Error = anyhow::Error;

    fn try_from(value: ButlerAction) -> Result<Self, Self::Error> {
        let response = value
            .response
            .as_ref()
            .and_then(|o| serde_json::to_string(o).ok());
        Ok(Self {
            id: value.id.to_string(),
            created_at: value.created_at,
            external_prompt: value.external_prompt,
            external_summary: value.external_summary,
            handler: value.handler.to_string(),
            handler_prompt: value.handler_prompt,
            snapshot_before: value.snapshot_before.to_string(),
            snapshot_after: value.snapshot_after.to_string(),
            response,
            error: value.error,
        })
    }
}

impl ButlerAction {
    pub fn new(
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
            handler,
            external_prompt,
            external_summary,
            handler_prompt: None,
            snapshot_before,
            snapshot_after,
            response: rsp.cloned(),
            error,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionListing {
    pub total: i64,
    pub actions: Vec<ButlerAction>,
}
