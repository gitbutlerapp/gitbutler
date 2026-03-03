//! Agent-state queries and updates.

use std::collections::BTreeMap;

use anyhow::Context;
use rusqlite::{Connection, Transaction, params};

use super::{AgentSnapshot, StaleAgent, coord_stale_threshold_ms, load_active_claims, now_unix_ms};

/// Ensure an agent row exists.
pub(crate) fn ensure_agent_row(conn: &Connection, agent_id: &str) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO agent_state(
            agent_id,
            status,
            plan,
            last_seen_at_ms,
            last_progress_at_ms
         ) VALUES (?1, NULL, NULL, 0, 0)
         ON CONFLICT(agent_id) DO NOTHING",
        params![agent_id],
    )
    .context("ensure_agent_row")?;
    Ok(())
}

/// Record that an agent was seen executing any command.
pub(crate) fn touch_agent_seen(conn: &Connection, agent_id: &str) -> anyhow::Result<()> {
    let now_ms = now_unix_ms()?;
    ensure_agent_row(conn, agent_id)?;
    conn.execute(
        "UPDATE agent_state SET last_seen_at_ms = ?2 WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;
    Ok(())
}

/// Record that an agent made coordination progress at a specific timestamp.
pub(crate) fn touch_agent_progress_at(
    conn: &Connection,
    agent_id: &str,
    now_ms: i64,
) -> anyhow::Result<()> {
    ensure_agent_row(conn, agent_id)?;
    conn.execute(
        "UPDATE agent_state
         SET last_seen_at_ms = ?2,
             last_progress_at_ms = ?2
         WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;
    Ok(())
}

/// Record that an agent made coordination progress inside an existing transaction.
pub(crate) fn touch_agent_progress_tx(
    tx: &Transaction<'_>,
    agent_id: &str,
    now_ms: i64,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO agent_state(
            agent_id,
            status,
            plan,
            last_seen_at_ms,
            last_progress_at_ms
         ) VALUES (?1, NULL, NULL, 0, 0)
         ON CONFLICT(agent_id) DO NOTHING",
        params![agent_id],
    )?;
    tx.execute(
        "UPDATE agent_state
         SET last_seen_at_ms = ?2,
             last_progress_at_ms = ?2
         WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;
    Ok(())
}

/// Load agent state snapshots ordered by recent progress.
pub(crate) fn load_agent_snapshots(conn: &Connection) -> anyhow::Result<Vec<AgentSnapshot>> {
    let mut stmt = conn.prepare(
        "SELECT agent_id, status, plan, last_seen_at_ms, last_progress_at_ms
         FROM agent_state
         ORDER BY last_progress_at_ms DESC, agent_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AgentSnapshot {
            agent_id: row.get(0)?,
            status: row.get(1)?,
            plan: row.get(2)?,
            last_seen_at_ms: row.get(3)?,
            last_progress_at_ms: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Load path-filtered stale claim holders.
pub(crate) fn stale_agents_for_paths(
    conn: &Connection,
    paths: &[String],
    now_ms: i64,
) -> anyhow::Result<Vec<StaleAgent>> {
    let threshold_ms = coord_stale_threshold_ms();
    if threshold_ms <= 0 {
        return Ok(Vec::new());
    }

    let claims = load_active_claims(conn, None)?;
    let mut claims_by_agent = BTreeMap::<String, Vec<String>>::new();
    for claim in claims {
        if !paths.is_empty()
            && !paths
                .iter()
                .any(|path| super::paths_overlap(path, &claim.path))
        {
            continue;
        }
        claims_by_agent
            .entry(claim.agent_id)
            .or_default()
            .push(claim.path);
    }

    let snapshots = load_agent_snapshots(conn)?;
    let mut stale = Vec::new();
    for snapshot in snapshots {
        let Some(claim_paths) = claims_by_agent.remove(&snapshot.agent_id) else {
            continue;
        };
        let stale_for_ms = now_ms.saturating_sub(snapshot.last_progress_at_ms);
        if stale_for_ms < threshold_ms {
            continue;
        }
        stale.push(StaleAgent {
            kind: "stale_agent",
            agent_id: snapshot.agent_id,
            last_progress_at_ms: snapshot.last_progress_at_ms,
            stale_for_ms,
            threshold_ms,
            is_stale: true,
            claim_paths,
        });
    }
    Ok(stale)
}
