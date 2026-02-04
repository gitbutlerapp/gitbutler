//! Release command implementation.

use chrono::Utc;
use serde::Serialize;

use super::hook_common;
use crate::db::DbHandle;
use crate::types::validate_agent_id;

/// Response for release command.
#[derive(Debug, Serialize)]
pub struct ReleaseResult {
    pub released: usize,
    pub agent_id: String,
}

/// Release file claims for an agent.
pub fn execute(db: &DbHandle, paths: Vec<String>, agent_id: String, all: bool) -> anyhow::Result<ReleaseResult> {
    validate_agent_id(&agent_id)?;

    db.upsert_agent(&agent_id, Utc::now())?;

    let released = if all {
        db.release_all_for_agent(&agent_id)?
    } else {
        if paths.is_empty() {
            anyhow::bail!("provide file paths to release, or use --all");
        }
        // Normalize paths to match what was stored.
        let repo_root = hook_common::find_repo_root();
        let normalized: Vec<String> = paths
            .iter()
            .map(|p| match &repo_root {
                Some(root) => hook_common::normalize_path(p, root),
                None => p.clone(),
            })
            .collect();
        let path_refs: Vec<&str> = normalized.iter().map(|s| s.as_str()).collect();
        db.release_files(&path_refs, &agent_id)?
    };

    Ok(ReleaseResult { released, agent_id })
}
