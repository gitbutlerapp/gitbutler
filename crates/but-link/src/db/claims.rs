//! Claim queries used by check, claim, and acquire flows.

use rusqlite::{Connection, OptionalExtension, params};

use super::{ActiveClaim, PrepareSql, SelfClaimState, now_unix_ms};

/// Load all active claims, optionally filtered by path prefix overlap.
pub(crate) fn load_active_claims(
    conn: &Connection,
    path_prefix: Option<&str>,
) -> anyhow::Result<Vec<ActiveClaim>> {
    let now_ms = now_unix_ms()?;
    let claims = if let Some(prefix) = path_prefix {
        let mut stmt = conn.prepare(
            "SELECT path, agent_id, expires_at_ms FROM claims
             WHERE expires_at_ms > ?1
               AND (path = ?2
                    OR substr(?2, 1, length(path) + 1) = path || '/'
                    OR substr(path, 1, length(?2) + 1) = ?2 || '/')
             ORDER BY expires_at_ms ASC, path ASC",
        )?;
        let rows = stmt.query_map(params![now_ms, prefix], |row| {
            Ok(ActiveClaim {
                path: row.get(0)?,
                agent_id: row.get(1)?,
                expires_at_ms: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        let mut stmt = conn.prepare(
            "SELECT path, agent_id, expires_at_ms FROM claims
             WHERE expires_at_ms > ?1
             ORDER BY expires_at_ms ASC, path ASC",
        )?;
        let rows = stmt.query_map(params![now_ms], |row| {
            Ok(ActiveClaim {
                path: row.get(0)?,
                agent_id: row.get(1)?,
                expires_at_ms: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(claims)
}

/// Load active claims owned by a specific agent.
pub(crate) fn load_active_claims_for_agent(
    conn: &Connection,
    agent_id: &str,
) -> anyhow::Result<Vec<ActiveClaim>> {
    let now_ms = now_unix_ms()?;
    let mut stmt = conn.prepare(
        "SELECT path, agent_id, expires_at_ms FROM claims
         WHERE agent_id = ?1 AND expires_at_ms > ?2
         ORDER BY expires_at_ms ASC, path ASC",
    )?;
    let rows = stmt.query_map(params![agent_id, now_ms], |row| {
        Ok(ActiveClaim {
            path: row.get(0)?,
            agent_id: row.get(1)?,
            expires_at_ms: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Determine the requester's current claim state using a generic SQL handle.
pub(crate) fn load_self_claim_state_with_handle(
    conn: &impl PrepareSql,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Option<SelfClaimState>> {
    load_self_claim_state_on(conn, agent_id, path, now_ms)
}

/// Determine the requester's current claim state using a connection or transaction.
fn load_self_claim_state_on(
    conn: &impl PrepareSql,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Option<SelfClaimState>> {
    let mut active_stmt = conn.prepare_query(
        "SELECT path, expires_at_ms FROM claims
         WHERE agent_id = ?1 AND expires_at_ms > ?2
           AND (path = ?3
                OR substr(?3, 1, length(path) + 1) = path || '/'
                OR substr(path, 1, length(?3) + 1) = ?3 || '/')
         ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC
         LIMIT 1",
    )?;
    let active = active_stmt
        .query_row(params![agent_id, now_ms, path], |row| {
            Ok(SelfClaimState {
                status: "active",
                path: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        })
        .optional()?;
    if active.is_some() {
        return Ok(active);
    }

    let mut stale_stmt = conn.prepare_query(
        "SELECT path, expires_at_ms FROM claims
         WHERE agent_id = ?1 AND expires_at_ms <= ?2
           AND (path = ?3
                OR substr(?3, 1, length(path) + 1) = path || '/'
                OR substr(path, 1, length(?3) + 1) = ?3 || '/')
         ORDER BY expires_at_ms DESC, LENGTH(path) DESC, path ASC
         LIMIT 1",
    )?;
    stale_stmt
        .query_row(params![agent_id, now_ms, path], |row| {
            Ok(SelfClaimState {
                status: "stale",
                path: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        })
        .optional()
        .map_err(Into::into)
}

/// Query claim conflicts using a generic SQL handle.
pub(crate) fn claim_conflicts_with_handle(
    conn: &impl PrepareSql,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<ActiveClaim>> {
    claim_conflicts_on(conn, agent_id, path, now_ms)
}

/// Query claim conflicts using either a connection or a transaction.
fn claim_conflicts_on(
    conn: &impl PrepareSql,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<ActiveClaim>> {
    let mut stmt = conn.prepare_query(
        "SELECT path, agent_id, expires_at_ms FROM claims
         WHERE agent_id <> ?2 AND expires_at_ms > ?3
           AND (path = ?1
                OR substr(?1, 1, length(path) + 1) = path || '/'
                OR substr(path, 1, length(?1) + 1) = ?1 || '/')
         ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC",
    )?;
    let rows = stmt.query_map(params![path, agent_id, now_ms], |row| {
        Ok(ActiveClaim {
            path: row.get(0)?,
            agent_id: row.get(1)?,
            expires_at_ms: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
