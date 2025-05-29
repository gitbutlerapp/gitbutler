use serde::{Deserialize, Serialize};
use strum::EnumString;

use crate::HandleChangesResponse;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, EnumString, Default)]
pub enum AutoHandler {
    #[default]
    HandleChangesSimple,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButCheckpoint {
    created_at: std::time::SystemTime,
    handler: AutoHandler,
    change_description: String,
    snapshot_before: gix::ObjectId,
    snapshot_after: gix::ObjectId,
    response: Option<HandleChangesResponse>,
    error: Option<String>,
}

impl ButCheckpoint {
    pub fn new(
        handler: AutoHandler,
        change_description: String,
        snapshot_before: gix::ObjectId,
        snapshot_after: gix::ObjectId,
        response: &anyhow::Result<HandleChangesResponse>,
    ) -> Self {
        let (rsp, error) = if let Err(e) = response {
            (None, Some(e.to_string()))
        } else {
            (response.as_ref().ok(), None)
        };

        Self {
            created_at: std::time::SystemTime::now(),
            handler,
            change_description,
            snapshot_before,
            snapshot_after,
            response: rsp.cloned(),
            error,
        }
    }
}

#[allow(unused)]
pub(crate) fn persist_checkpoint(checkpoint: ButCheckpoint) -> anyhow::Result<()> {
    // TODO
    Ok(())
}
