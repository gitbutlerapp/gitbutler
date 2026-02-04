//! Plan command implementation.
//!
//! Sets or clears an agent's current plan. Plans are visible to other
//! agents via hook summaries, enabling pre-execution coordination —
//! teammates can flag conflicts before work begins.

use chrono::Utc;

use crate::db::DbHandle;
use crate::types::{Agent, validate_agent_id};

/// Maximum length for a plan.
pub const MAX_PLAN_LEN: usize = 4096;

/// Set or clear the agent's plan.
pub fn execute(db: &DbHandle, agent_id: String, plan: Option<String>, clear: bool) -> anyhow::Result<Agent> {
    validate_agent_id(&agent_id)?;

    if clear && plan.is_some() {
        anyhow::bail!("cannot use both --clear and a plan message");
    }

    let now = Utc::now();
    db.upsert_agent(&agent_id, now)?;

    if clear {
        db.set_agent_plan(&agent_id, None, now)?;
    } else if let Some(ref p) = plan {
        if p.len() > MAX_PLAN_LEN {
            anyhow::bail!("plan exceeds maximum length of {MAX_PLAN_LEN}");
        }
        db.set_agent_plan(&agent_id, Some(p), now)?;
    }

    let agent = db.get_agent(&agent_id)?.unwrap();
    Ok(agent)
}
