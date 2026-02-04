//! Done command implementation.
//!
//! Announces task completion and performs cleanup in one step:
//! - release all claims for the agent
//! - clear the agent plan
//! - post a completion message to the shared channel

use chrono::Utc;
use serde::Serialize;

use super::post;
use crate::db::DbHandle;
use crate::types::{Message, validate_agent_id, validate_content};

/// Response for done command.
#[derive(Debug, Serialize)]
pub struct DoneResult {
    pub message: Message,
    pub released: usize,
    pub plan_cleared: bool,
    pub agent_id: String,
}

/// Complete the current task for an agent, announce completion, and clean up
/// coordination state.
pub fn execute(db: &DbHandle, summary: String, agent_id: String) -> anyhow::Result<DoneResult> {
    validate_agent_id(&agent_id)?;
    validate_content(&summary)?;

    let now = Utc::now();
    db.upsert_agent(&agent_id, now)?;

    let released = db.release_all_for_agent(&agent_id)?;
    db.set_agent_plan(&agent_id, None, now)?;

    let content = format!("DONE: {summary}");
    let message = post::execute(db, content, agent_id.clone())?;

    Ok(DoneResult {
        message,
        released,
        plan_cleared: true,
        agent_id,
    })
}
