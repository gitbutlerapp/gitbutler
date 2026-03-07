//! Claim, check, and acquire workflows for `but link`.

use std::collections::BTreeMap;
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::CheckFormat;
use crate::db;
use crate::payloads::{AcquireHistory, ClaimHistory};
use crate::repo::normalized_unique_paths;

/// Check response describing path coordination state.
#[derive(Serialize)]
pub(crate) struct CheckResponse {
    /// Checked path.
    pub path: String,
    /// `allow`, `warn`, or `deny`.
    pub decision: &'static str,
    /// Stable reason code for machine consumers.
    pub reason_code: &'static str,
    /// Current claim state for the requester.
    pub self_claim: Value,
    /// Conflicting claims from other agents.
    pub blocking_claims: Vec<db::ActiveClaim>,
    /// Relevant hard typed blocks.
    pub typed_blocks: Vec<db::TypedBlock>,
    /// Relevant advisories (advisory blocks and path discoveries).
    pub advisories: Vec<Value>,
    /// Relevant dependency hints.
    pub dependency_hints: Vec<db::DependencyHint>,
    /// Stale claim holders relevant to this path.
    pub stale_agents: Vec<db::StaleAgent>,
}

/// Per-path acquisition outcome.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AcquireDecision {
    /// Path evaluated for acquisition.
    pub path: String,
    /// `acquired` or `blocked`.
    pub decision: &'static str,
    /// Stable reason code for machine consumers.
    pub reason_code: &'static str,
    /// Claim blockers from other agents.
    pub blocking_claims: Vec<db::ActiveClaim>,
    /// Relevant hard typed blocks.
    pub typed_blocks: Vec<db::TypedBlock>,
    /// Relevant advisories (advisory blocks and path discoveries).
    pub advisories: Vec<Value>,
    /// Final claim row when acquired.
    pub claim: Option<db::ActiveClaim>,
}

/// Aggregated acquire response payload.
#[derive(Debug, Serialize)]
pub(crate) struct AcquireResponse {
    /// Standard success marker.
    pub ok: bool,
    /// Paths successfully acquired.
    pub acquired_paths: Vec<String>,
    /// Paths that remained blocked.
    pub blocked_paths: Vec<String>,
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

/// Claim the provided paths without conflict checks.
pub(crate) fn claim_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: Duration,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let ttl_ms: i64 = ttl.as_millis().try_into()?;
    let expires_at_ms = now_ms.saturating_add(ttl_ms);
    let normalized = normalized_unique_paths(paths);
    let tx = conn.transaction()?;
    {
        let mut delete_stmt = tx.prepare("DELETE FROM claims WHERE path = ?1 AND agent_id = ?2")?;
        let mut insert_stmt =
            tx.prepare("INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)")?;
        for path in &normalized {
            delete_stmt.execute(params![path, agent_id])?;
            insert_stmt.execute(params![path, agent_id, expires_at_ms])?;
        }
    }
    let body = ClaimHistory {
        text: format!("claimed: {}", normalized.join(", ")),
        paths: normalized,
    };
    db::insert_history_message_tx(
        &tx,
        now_ms,
        agent_id,
        "claim",
        &serde_json::to_string(&body)?,
    )?;
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    Ok(())
}

/// Release the provided paths.
pub(crate) fn release_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
) -> anyhow::Result<()> {
    let normalized = normalized_unique_paths(paths);
    let tx = conn.transaction()?;
    {
        let mut delete_stmt = tx.prepare("DELETE FROM claims WHERE path = ?1 AND agent_id = ?2")?;
        for path in &normalized {
            delete_stmt.execute(params![path, agent_id])?;
        }
    }
    let now_ms = db::now_unix_ms()?;
    let body = ClaimHistory {
        text: format!("released: {}", normalized.join(", ")),
        paths: normalized,
    };
    db::insert_history_message_tx(
        &tx,
        now_ms,
        agent_id,
        "release",
        &serde_json::to_string(&body)?,
    )?;
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    Ok(())
}

/// Build read-only check results for one or more paths.
pub(crate) fn check_results(
    conn: &Connection,
    agent_id: &str,
    paths: &[String],
    strict: bool,
) -> anyhow::Result<Vec<CheckResponse>> {
    paths
        .iter()
        .map(|path| check_result(conn, agent_id, path, strict))
        .collect()
}

/// Build the read-only check result for a single path.
pub(crate) fn check_result(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    strict: bool,
) -> anyhow::Result<CheckResponse> {
    let path = crate::cli::normalize_claim_path(path)?;
    let now_ms = db::now_unix_ms()?;
    let self_claim = db::load_self_claim_state(conn, agent_id, &path, now_ms)?;
    let blocking_claims = db::claim_conflicts(conn, agent_id, &path, now_ms)?;
    let relevant_blocks = db::load_open_blocks(conn, Some(agent_id), Some(&path))?;
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
        std::slice::from_ref(&path),
        5,
    )?);
    let dependency_hints =
        db::dependency_hints_for_paths(conn, agent_id, std::slice::from_ref(&path))?;
    let stale_agents = db::stale_agents_for_paths(conn, std::slice::from_ref(&path), now_ms)?;

    let (decision, reason_code) = if !blocking_claims.is_empty() {
        if strict {
            ("deny", "claimed_by_other")
        } else {
            ("warn", "claimed_by_other")
        }
    } else if !typed_blocks.is_empty() {
        ("deny", "hard_block")
    } else if !relevant_blocks.is_empty() {
        if strict {
            ("deny", "advisory_block")
        } else {
            ("warn", "advisory_block")
        }
    } else {
        ("allow", "no_conflict")
    };

    Ok(CheckResponse {
        path,
        decision,
        reason_code,
        self_claim: self_claim_json(self_claim),
        blocking_claims,
        typed_blocks,
        advisories,
        dependency_hints,
        stale_agents,
    })
}

/// Acquire the provided paths transactionally with partial success across the batch.
pub(crate) fn acquire_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: Duration,
    strict: bool,
) -> anyhow::Result<AcquireResponse> {
    let now_ms = db::now_unix_ms()?;
    let ttl_ms: i64 = ttl.as_millis().try_into()?;
    let expires_at_ms = now_ms.saturating_add(ttl_ms);
    let normalized = normalized_unique_paths(paths);
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut decisions = Vec::new();
    let mut acquired_paths = Vec::new();

    for path in &normalized {
        let blocking_claims = db::claim_conflicts_tx(&tx, agent_id, path, now_ms)?;
        let relevant_blocks = db::load_open_blocks_tx(&tx, Some(agent_id), Some(path), now_ms)?;
        let hard_blocks: Vec<db::TypedBlock> = relevant_blocks
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
            &tx,
            std::slice::from_ref(path),
            5,
        )?);

        let (decision, reason_code) = if !blocking_claims.is_empty() {
            ("blocked", "claimed_by_other")
        } else if !hard_blocks.is_empty() {
            ("blocked", "hard_block")
        } else if strict && relevant_blocks.iter().any(|block| block.mode == "advisory") {
            ("blocked", "advisory_block")
        } else if relevant_blocks.iter().any(|block| block.mode == "advisory") {
            ("acquired", "advisory_block")
        } else {
            ("acquired", "no_conflict")
        };

        let claim = if decision == "acquired" {
            tx.execute(
                "DELETE FROM claims WHERE path = ?1 AND agent_id = ?2",
                params![path, agent_id],
            )?;
            tx.execute(
                "INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)",
                params![path, agent_id, expires_at_ms],
            )?;
            acquired_paths.push(path.clone());
            Some(db::ActiveClaim {
                path: path.clone(),
                agent_id: agent_id.to_owned(),
                expires_at_ms,
            })
        } else {
            None
        };

        decisions.push(AcquireDecision {
            path: path.clone(),
            decision,
            reason_code,
            blocking_claims,
            typed_blocks: hard_blocks,
            advisories,
            claim,
        });
    }

    if !acquired_paths.is_empty()
        || decisions
            .iter()
            .any(|decision| decision.decision == "blocked")
    {
        let body = AcquireHistory {
            text: if acquired_paths.is_empty() {
                format!("acquire blocked: {}", normalized.join(", "))
            } else {
                format!("acquired: {}", acquired_paths.join(", "))
            },
            paths: normalized.clone(),
            acquired_paths: acquired_paths.clone(),
        };
        db::insert_history_message_tx(
            &tx,
            now_ms,
            agent_id,
            "acquire",
            &serde_json::to_string(&body)?,
        )?;
    }

    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;

    let active_claims = db::load_active_claims_for_agent(conn, agent_id)?;
    let typed_blocks = collect_unique_blocks(
        decisions
            .iter()
            .flat_map(|decision| decision.typed_blocks.clone())
            .collect(),
    );
    let dependency_hints = db::dependency_hints_for_paths(conn, agent_id, &normalized)?;
    let stale_agents = db::stale_agents_for_paths(conn, &normalized, now_ms)?;
    let blocked_paths: Vec<String> = decisions
        .iter()
        .filter(|decision| decision.decision == "blocked")
        .map(|decision| decision.path.clone())
        .collect();
    Ok(AcquireResponse {
        ok: true,
        acquired_paths: active_claims
            .iter()
            .map(|claim| claim.path.clone())
            .filter(|path| normalized.contains(path))
            .collect(),
        blocked_paths,
        decisions,
        active_claims,
        typed_blocks,
        dependency_hints,
        stale_agents,
    })
}

/// Render compact output lines for check or acquire results.
pub(crate) fn compact_lines_for_check(
    results: &[CheckResponse],
    format: CheckFormat,
) -> Option<Vec<String>> {
    match format {
        CheckFormat::Full => None,
        CheckFormat::Compact => Some(
            results
                .iter()
                .map(|result| {
                    let blockers = blocker_summary(&result.blocking_claims, &result.typed_blocks);
                    format!(
                        "{} {} {}{}",
                        result.decision, result.path, result.reason_code, blockers
                    )
                })
                .collect(),
        ),
    }
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
                        "{} {} {}{}",
                        decision.decision, decision.path, decision.reason_code, blockers
                    )
                })
                .collect(),
        ),
    }
}

/// Build a compact blocker summary string.
pub(crate) fn blocker_summary(
    blocking_claims: &[db::ActiveClaim],
    typed_blocks: &[db::TypedBlock],
) -> String {
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
        format!(" {}", blockers.join(","))
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};

    use super::*;

    /// Create a temporary database directory for concurrency tests.
    fn temp_test_dir(name: &str) -> anyhow::Result<PathBuf> {
        let unique = format!(
            "but-link-{}-{}-{}",
            name,
            std::process::id(),
            db::now_unix_ms()?
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    #[test]
    fn claim_batch_is_atomic_with_message_insert() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        conn.execute(
            "CREATE TRIGGER fail_claim_message BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL, 'boom'); END;",
            [],
        )?;

        let err = claim_batch(
            &mut conn,
            "agent-a",
            &[String::from("src/lib.rs"), String::from("src/main.rs")],
            Duration::from_secs(60),
        )
        .expect_err("message trigger should fail");

        assert!(err.to_string().contains("boom"));
        let claim_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM claims", [], |row| row.get(0))?;
        assert_eq!(claim_count, 0);
        Ok(())
    }

    #[test]
    fn release_batch_is_atomic_with_message_insert() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        let now_ms = db::now_unix_ms()?;
        conn.execute(
            "INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)",
            params!["src/lib.rs", "agent-a", now_ms + 60_000],
        )?;
        conn.execute(
            "CREATE TRIGGER fail_release_message BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL, 'boom'); END;",
            [],
        )?;

        let err = release_batch(&mut conn, "agent-a", &[String::from("src/lib.rs")])
            .expect_err("message trigger should fail");

        assert!(err.to_string().contains("boom"));
        let claim_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM claims", [], |row| row.get(0))?;
        assert_eq!(claim_count, 1);
        Ok(())
    }

    #[test]
    fn check_ignores_free_text_blocking_words() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        db::insert_history_message(
            &conn,
            db::now_unix_ms()?,
            "peer",
            "message",
            &json!({ "text": "please avoid src/app.txt while I refactor" }).to_string(),
        )?;

        let result = check_result(&conn, "me", "src/app.txt", false)?;
        assert_eq!(result.decision, "allow");
        assert_eq!(result.reason_code, "no_conflict");
        Ok(())
    }

    #[test]
    fn check_respects_typed_blocks() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        conn.execute(
            "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
             VALUES ('peer', 'hard', 'shared refactor', 10, NULL, NULL, NULL)",
            [],
        )?;
        let block_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO block_paths(block_id, path) VALUES (?1, 'src/app.txt')",
            params![block_id],
        )?;

        let result = check_result(&conn, "me", "src/app.txt", false)?;
        assert_eq!(result.decision, "deny");
        assert_eq!(result.reason_code, "hard_block");
        Ok(())
    }

    #[test]
    fn acquire_claims_clear_paths_and_skips_blocked_paths() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        conn.execute(
            "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
             VALUES ('peer', 'hard', 'shared refactor', 10, NULL, NULL, NULL)",
            [],
        )?;
        let block_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO block_paths(block_id, path) VALUES (?1, 'src/blocked.txt')",
            params![block_id],
        )?;

        acquire_batch(
            &mut conn,
            "me",
            &[
                String::from("src/clear.txt"),
                String::from("src/blocked.txt"),
            ],
            Duration::from_secs(900),
            false,
        )?;

        let claims = db::load_active_claims_for_agent(&conn, "me")?;
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].path, "src/clear.txt");
        Ok(())
    }

    #[test]
    fn acquire_serializes_concurrent_writers_into_blocked_result() -> anyhow::Result<()> {
        let tempdir = temp_test_dir("acquire-concurrent")?;
        let db_path = tempdir.join("coord.db");

        let conn = Connection::open(&db_path)?;
        db::init_db(&conn)?;
        drop(conn);

        let barrier = Arc::new(Barrier::new(2));
        let db_path_a = db_path.clone();
        let barrier_a = barrier.clone();
        let thread_a = std::thread::spawn(move || -> anyhow::Result<AcquireResponse> {
            let mut conn = Connection::open(db_path_a)?;
            db::init_db(&conn)?;
            barrier_a.wait();
            acquire_batch(
                &mut conn,
                "agent-a",
                &[String::from("src/app.txt")],
                Duration::from_secs(900),
                false,
            )
        });

        let barrier_b = barrier.clone();
        let thread_b = std::thread::spawn(move || -> anyhow::Result<AcquireResponse> {
            let mut conn = Connection::open(db_path)?;
            db::init_db(&conn)?;
            barrier_b.wait();
            acquire_batch(
                &mut conn,
                "agent-b",
                &[String::from("src/app.txt")],
                Duration::from_secs(900),
                false,
            )
        });

        let response_a = thread_a.join().expect("thread a panicked")?;
        let response_b = thread_b.join().expect("thread b panicked")?;
        let decisions = [
            response_a.decisions[0].decision,
            response_b.decisions[0].decision,
        ];

        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == "acquired")
                .count(),
            1
        );
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == "blocked")
                .count(),
            1
        );

        let conn = Connection::open(tempdir.join("coord.db"))?;
        db::init_db(&conn)?;
        let claims = db::load_active_claims(&conn, Some("src/app.txt"))?;
        assert_eq!(claims.len(), 1);
        Ok(())
    }
}
