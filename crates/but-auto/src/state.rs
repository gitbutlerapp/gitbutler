use serde::{Deserialize, Serialize};
use strum::EnumString;

use crate::Outcome;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, EnumString, Default)]
pub enum AutoHandler {
    #[default]
    HandleChangesSimple,
}

/// Represents a snapshot of an automatic action taken by a GitButler automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButlerAction {
    /// The time when the action was performed.
    created_at: std::time::SystemTime,
    /// A description of the change that was made and why it was made - i.e. the information that can be obtained from the caller.
    external_prompt: String,
    /// The handler / implementation that performed the action.
    handler: AutoHandler,
    /// An optional prompt that was used by the handler to perform the action, if applicable.
    handler_prompt: Option<String>,
    /// A GitBulter Oplog snapshot ID before the action was performed.
    snapshot_before: gix::ObjectId,
    /// A GitBulter Oplog snapshot ID after the action was performed.
    snapshot_after: gix::ObjectId,
    /// The outcome of the action, if it was successful.
    response: Option<Outcome>,
    /// An error message if the action failed.
    error: Option<String>,
}

impl ButlerAction {
    pub fn new(
        handler: AutoHandler,
        external_prompt: String,
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
            created_at: std::time::SystemTime::now(),
            handler,
            external_prompt,
            handler_prompt: None,
            snapshot_before,
            snapshot_after,
            response: rsp.cloned(),
            error,
        }
    }
}

#[allow(unused)]
pub(crate) fn persist_checkpoint(action: ButlerAction) -> anyhow::Result<()> {
    // TODO
    Ok(())
}
