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
    response: HandleChangesResponse,
}

impl ButCheckpoint {
    pub fn new(
        handler: AutoHandler,
        change_description: String,
        snapshot_before: gix::ObjectId,
        snapshot_after: gix::ObjectId,
        response: HandleChangesResponse,
    ) -> Self {
        Self {
            created_at: std::time::SystemTime::now(),
            handler,
            change_description,
            snapshot_before,
            snapshot_after,
            response,
        }
    }
}

#[allow(unused)]
pub(crate) fn persist_checkpoint(checkpoint: ButCheckpoint) -> anyhow::Result<()> {
    // TODO
    Ok(())
}
