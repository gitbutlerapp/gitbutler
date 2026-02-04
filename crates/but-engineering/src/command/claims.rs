//! Claims listing command implementation.

use chrono::Utc;

use crate::db::DbHandle;
use crate::duration::parse_duration;
use crate::types::Claim;

/// List active file claims, optionally filtered by agent activity.
pub fn execute(db: &DbHandle, active_within: Option<String>) -> anyhow::Result<Vec<Claim>> {
    let active_since = match active_within {
        Some(ref dur_str) => {
            let duration = parse_duration(dur_str)?;
            let chrono_duration =
                chrono::Duration::from_std(duration).map_err(|e| anyhow::anyhow!("duration too large: {e}"))?;
            Some(Utc::now() - chrono_duration)
        }
        None => None,
    };

    let claims = db.list_claims(active_since)?;
    Ok(claims)
}
