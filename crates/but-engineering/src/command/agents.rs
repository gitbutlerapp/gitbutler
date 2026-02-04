//! Agents command implementation.

use chrono::Utc;

use crate::db::DbHandle;
use crate::duration::parse_duration;
use crate::types::AgentInfo;

/// List active agents.
pub fn execute(db: &DbHandle, active_within: Option<String>) -> anyhow::Result<Vec<AgentInfo>> {
    let active_since = match active_within {
        Some(ref duration_str) => {
            let duration = parse_duration(duration_str)?;
            let chrono_duration =
                chrono::Duration::from_std(duration).map_err(|e| anyhow::anyhow!("duration too large: {e}"))?;
            Some(Utc::now() - chrono_duration)
        }
        None => None,
    };

    let agents = db.list_agents(active_since)?;

    // Convert to AgentInfo (without last_read)
    Ok(agents.into_iter().map(AgentInfo::from).collect())
}
