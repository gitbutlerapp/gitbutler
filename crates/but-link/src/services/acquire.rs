//! Service handlers for acquisition workflows.

use std::collections::BTreeMap;
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::CheckFormat;
use crate::db;
use crate::repo::normalized_unique_paths;

/// Default backoff before retrying a blocked acquire decision.
const DEFAULT_RETRY_AFTER_MS: i64 = 30_000;

/// Per-path acquisition outcome.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AcquireDecision {
    /// Path evaluated for acquisition.
    pub path: String,
    /// One of `acquired`, `blocked`, `allow`, `warn`, or `deny`.
    pub decision: &'static str,
    /// Stable reason code for machine consumers.
    pub reason_code: &'static str,
    /// Current claim state for the requester.
    pub self_claim: Value,
    /// Claim blockers from other agents.
    pub blocking_claims: Vec<db::ActiveClaim>,
    /// Relevant hard typed blocks.
    pub typed_blocks: Vec<db::TypedBlock>,
    /// Relevant advisories.
    pub advisories: Vec<Value>,
    /// Suggested delay before retrying a blocked path.
    pub retry_after_ms: Option<i64>,
    /// Suggested unix timestamp for the next retry attempt.
    pub retry_at_ms: Option<i64>,
    /// Final claim row when acquired.
    pub claim: Option<db::ActiveClaim>,
}

/// Aggregated acquire response payload.
#[derive(Debug, Serialize)]
pub(crate) struct AcquireResponse {
    /// Standard success marker.
    pub ok: bool,
    /// Whether the command ran in read-only mode.
    pub dry_run: bool,
    /// Paths successfully acquired.
    pub acquired_paths: Vec<String>,
    /// Paths that remained blocked.
    pub blocked_paths: Vec<String>,
    /// Paths that produced a non-blocking warning during dry-run.
    pub warn_paths: Vec<String>,
    /// Per-path final outcomes.
    pub decisions: Vec<AcquireDecision>,
    /// Requester claims after acquisition.
    pub active_claims: Vec<db::ActiveClaim>,
    /// Relevant typed blocks across the requested paths.
    pub typed_blocks: Vec<db::TypedBlock>,
    /// Relevant dependency hints.
    pub dependency_hints: Vec<db::DependencyHint>,
    /// Relevant stale claim holders.
    pub stale_agents: Vec<db::StaleAgent>,
}

/// Acquire or dry-run the provided paths transactionally with partial success across the batch.
pub(crate) fn acquire_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: Duration,
    strict: bool,
    dry_run: bool,
) -> anyhow::Result<AcquireResponse> {
    let now_ms = db::now_unix_ms()?;
    let ttl_ms: i64 = ttl.as_millis().try_into()?;
    let expires_at_ms = now_ms.saturating_add(ttl_ms);
    let normalized = normalized_unique_paths(paths);

    if dry_run {
        let decisions: Vec<AcquireDecision> = normalized
            .iter()
            .map(|path| evaluate_path(conn, agent_id, path, now_ms, strict, false, expires_at_ms))
            .collect::<anyhow::Result<Vec<_>>>()?;
        return build_response(conn, agent_id, &normalized, decisions, true, now_ms);
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut decisions = Vec::new();
    for path in &normalized {
        decisions.push(evaluate_path(
            &tx,
            agent_id,
            path,
            now_ms,
            strict,
            true,
            expires_at_ms,
        )?);
    }
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;

    build_response(conn, agent_id, &normalized, decisions, false, now_ms)
}

/// Render compact output lines for acquire results.
pub(crate) fn compact_lines_for_acquire(
    response: &AcquireResponse,
    format: CheckFormat,
) -> Option<Vec<String>> {
    match format {
        CheckFormat::Full => None,
        CheckFormat::Compact => Some(
            response
                .decisions
                .iter()
                .map(|decision| {
                    let blockers =
                        blocker_summary(&decision.blocking_claims, &decision.typed_blocks);
                    format!(
                        "{decision_value} {path} {reason_code}{blockers}",
                        decision_value = decision.decision,
                        path = decision.path,
                        reason_code = decision.reason_code,
                    )
                })
                .collect(),
        ),
    }
}

/// Convert a typed block into an advisory JSON object.
pub(crate) fn block_to_advisory_value(block: &db::TypedBlock) -> Value {
    json!({
        "kind": "block",
        "id": block.id,
        "agent_id": block.agent_id,
        "mode": block.mode,
        "reason": block.reason,
        "paths": block.paths,
        "created_at_ms": block.created_at_ms,
        "expires_at_ms": block.expires_at_ms,
    })
}

/// Evaluate one path and optionally acquire it.
fn evaluate_path(
    conn: &impl db::PrepareSql,
    agent_id: &str,
    path: &str,
    now_ms: i64,
    strict: bool,
    mutate: bool,
    expires_at_ms: i64,
) -> anyhow::Result<AcquireDecision> {
    let self_claim = db::load_self_claim_state_with_handle(conn, agent_id, path, now_ms)?;
    let blocking_claims = db::claim_conflicts_with_handle(conn, agent_id, path, now_ms)?;
    let relevant_blocks =
        db::load_open_blocks_with_handle(conn, Some(agent_id), Some(path), now_ms)?;
    let typed_blocks: Vec<db::TypedBlock> = relevant_blocks
        .iter()
        .filter(|block| block.mode == "hard")
        .cloned()
        .collect();
    let mut advisories: Vec<Value> = relevant_blocks
        .iter()
        .filter(|block| block.mode == "advisory")
        .map(block_to_advisory_value)
        .collect();
    advisories.extend(db::recent_discoveries_for_paths(
        conn,
        std::slice::from_ref(&path.to_owned()),
        5,
    )?);
    let (decision, reason_code) = if !blocking_claims.is_empty() {
        if mutate {
            ("blocked", "claimed_by_other")
        } else if strict {
            ("deny", "claimed_by_other")
        } else {
            ("warn", "claimed_by_other")
        }
    } else if !typed_blocks.is_empty() {
        if mutate {
            ("blocked", "hard_block")
        } else {
            ("deny", "hard_block")
        }
    } else if relevant_blocks.iter().any(|block| block.mode == "advisory") {
        if mutate {
            if strict {
                ("blocked", "advisory_block")
            } else {
                ("acquired", "advisory_block")
            }
        } else if strict {
            ("deny", "advisory_block")
        } else {
            ("warn", "advisory_block")
        }
    } else if mutate {
        ("acquired", "no_conflict")
    } else {
        ("allow", "no_conflict")
    };

    let claim = if mutate && decision == "acquired" {
        conn.prepare_query("DELETE FROM claims WHERE path = ?1 AND agent_id = ?2")?
            .execute(params![path, agent_id])?;
        conn.prepare_query(
            "INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)",
        )?
        .execute(params![path, agent_id, expires_at_ms])?;
        Some(db::ActiveClaim {
            path: path.to_owned(),
            agent_id: agent_id.to_owned(),
            expires_at_ms,
        })
    } else {
        None
    };
    let retry_after_ms =
        suggested_retry_after_ms(now_ms, &blocking_claims, &relevant_blocks, decision);
    let retry_at_ms = retry_after_ms.map(|delay_ms| now_ms.saturating_add(delay_ms));

    Ok(AcquireDecision {
        path: path.to_owned(),
        decision,
        reason_code,
        self_claim: self_claim_json(self_claim),
        blocking_claims,
        typed_blocks,
        advisories,
        retry_after_ms,
        retry_at_ms,
        claim,
    })
}

/// Build the final acquire response after mutation or dry-run.
fn build_response(
    conn: &Connection,
    agent_id: &str,
    normalized: &[String],
    decisions: Vec<AcquireDecision>,
    dry_run: bool,
    now_ms: i64,
) -> anyhow::Result<AcquireResponse> {
    let active_claims = db::load_active_claims_for_agent(conn, agent_id)?;
    let typed_blocks = collect_unique_blocks(
        decisions
            .iter()
            .flat_map(|decision| decision.typed_blocks.clone())
            .collect(),
    );
    let dependency_hints = db::dependency_hints_for_paths(conn, agent_id, normalized)?;
    let stale_agents = db::stale_agents_for_paths(conn, normalized, now_ms)?;
    let blocked_paths: Vec<String> = decisions
        .iter()
        .filter(|decision| matches!(decision.decision, "blocked" | "deny"))
        .map(|decision| decision.path.clone())
        .collect();
    let warn_paths: Vec<String> = decisions
        .iter()
        .filter(|decision| decision.decision == "warn")
        .map(|decision| decision.path.clone())
        .collect();
    let acquired_paths: Vec<String> = decisions
        .iter()
        .filter(|decision| matches!(decision.decision, "acquired" | "allow"))
        .map(|decision| decision.path.clone())
        .collect();
    Ok(AcquireResponse {
        ok: true,
        dry_run,
        acquired_paths,
        blocked_paths,
        warn_paths,
        decisions,
        active_claims,
        typed_blocks,
        dependency_hints,
        stale_agents,
    })
}

/// Build a compact blocker summary string.
fn blocker_summary(blocking_claims: &[db::ActiveClaim], typed_blocks: &[db::TypedBlock]) -> String {
    let mut blockers = Vec::new();
    for claim in blocking_claims {
        blockers.push(claim.agent_id.clone());
    }
    for block in typed_blocks {
        blockers.push(block.agent_id.clone());
    }
    blockers.sort();
    blockers.dedup();
    if blockers.is_empty() {
        String::new()
    } else {
        let joined = blockers.join(",");
        format!(" {joined}")
    }
}

/// Serialize requester claim state to JSON.
fn self_claim_json(claim: Option<db::SelfClaimState>) -> Value {
    if let Some(claim) = claim {
        json!({
            "status": claim.status,
            "path": claim.path,
            "expires_at_ms": claim.expires_at_ms,
        })
    } else {
        json!({ "status": "none" })
    }
}

/// Deduplicate typed blocks by id.
fn collect_unique_blocks(blocks: Vec<db::TypedBlock>) -> Vec<db::TypedBlock> {
    let mut grouped = BTreeMap::new();
    for block in blocks {
        grouped.entry(block.id).or_insert(block);
    }
    grouped.into_values().collect()
}

/// Return a bounded retry delay for blocked or denied acquisition outcomes.
fn suggested_retry_after_ms(
    now_ms: i64,
    blocking_claims: &[db::ActiveClaim],
    relevant_blocks: &[db::TypedBlock],
    decision: &str,
) -> Option<i64> {
    if !matches!(decision, "blocked" | "deny" | "warn") {
        return None;
    }

    let known_unblock_delta_ms = blocking_claims
        .iter()
        .map(|claim| claim.expires_at_ms)
        .chain(
            relevant_blocks
                .iter()
                .filter_map(|block| block.expires_at_ms),
        )
        .filter(|expires_at_ms| *expires_at_ms > now_ms)
        .map(|expires_at_ms| expires_at_ms.saturating_sub(now_ms))
        .min();

    Some(
        known_unblock_delta_ms
            .map(|delta_ms| delta_ms.min(DEFAULT_RETRY_AFTER_MS))
            .unwrap_or(DEFAULT_RETRY_AFTER_MS),
    )
}
