//! Claim command implementation.

use chrono::Utc;

use super::hook_common;
use crate::db::DbHandle;
use crate::types::{Claim, validate_agent_id};

/// Claim one or more files for an agent.
pub fn execute(db: &DbHandle, paths: Vec<String>, agent_id: String) -> anyhow::Result<Vec<Claim>> {
    validate_agent_id(&agent_id)?;
    if paths.is_empty() {
        anyhow::bail!("at least one file path is required");
    }

    let now = Utc::now();

    // Normalize paths to be relative to the repo root.
    let repo_root = hook_common::find_repo_root();
    let normalized: Vec<String> = paths
        .iter()
        .map(|p| match &repo_root {
            Some(root) => hook_common::normalize_path(p, root),
            None => p.clone(),
        })
        .collect();

    let path_refs: Vec<&str> = normalized.iter().map(|s| s.as_str()).collect();

    db.upsert_agent(&agent_id, now)?;
    db.claim_files(&path_refs, &agent_id, now)?;

    let claims: Vec<Claim> = normalized
        .into_iter()
        .map(|file_path| Claim {
            file_path,
            agent_id: agent_id.clone(),
            claimed_at: now,
        })
        .collect();

    Ok(claims)
}
