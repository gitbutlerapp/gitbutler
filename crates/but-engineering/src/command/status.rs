//! Status command implementation.

use chrono::Utc;

use crate::db::DbHandle;
use crate::types::{AgentInfo, validate_agent_id, validate_status};

/// Set, clear, or get agent status.
pub fn execute(
    db: &DbHandle,
    agent_id: String,
    status_message: Option<String>,
    clear: bool,
) -> anyhow::Result<AgentInfo> {
    validate_agent_id(&agent_id)?;
    if let Some(ref s) = status_message {
        validate_status(s)?;
    }

    // Check for conflicting options
    if clear && status_message.is_some() {
        anyhow::bail!("cannot use both --clear and provide a status message");
    }

    let now = Utc::now();

    db.upsert_agent(&agent_id, now)?;

    if clear {
        db.set_agent_status(&agent_id, None, now)?;
    } else if let Some(ref status) = status_message {
        db.set_agent_status(&agent_id, Some(status), now)?;
    }

    // Fetch and return the agent (as AgentInfo to exclude last_read)
    let agent = db
        .get_agent(&agent_id)?
        .ok_or_else(|| anyhow::anyhow!("agent not found after upsert"))?;
    Ok(AgentInfo::from(agent))
}
