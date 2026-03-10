//! Typed block and acknowledgement queries.

use std::collections::BTreeMap;

use rusqlite::{Connection, params};
use serde_json::json;

use super::{BlockRow, PrepareSql, TypedBlock, UnreadUpdate, now_unix_ms, paths_overlap};

/// Load open typed blocks, optionally filtered to overlaps with a path.
pub(crate) fn load_open_blocks(
    conn: &Connection,
    except_agent_id: Option<&str>,
    overlap_path: Option<&str>,
) -> anyhow::Result<Vec<TypedBlock>> {
    load_open_blocks_on(conn, except_agent_id, overlap_path, now_unix_ms()?)
}

/// Load open typed blocks using a generic SQL handle.
pub(crate) fn load_open_blocks_with_handle(
    conn: &impl PrepareSql,
    except_agent_id: Option<&str>,
    overlap_path: Option<&str>,
    now_ms: i64,
) -> anyhow::Result<Vec<TypedBlock>> {
    load_open_blocks_on(conn, except_agent_id, overlap_path, now_ms)
}

/// Check if an acknowledgement exists for the target and path scope.
pub(crate) fn ack_exists_since(
    conn: &Connection,
    from_agent_id: &str,
    target_agent_id: &str,
    path: &str,
    since_ms: i64,
) -> anyhow::Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT a.id, ap.path
         FROM acknowledgements a
         LEFT JOIN ack_paths ap ON ap.ack_id = a.id
         WHERE a.agent_id = ?1
           AND a.target_agent_id = ?2
           AND a.created_at_ms >= ?3
         ORDER BY a.id DESC",
    )?;
    let rows = stmt.query_map(params![from_agent_id, target_agent_id, since_ms], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    for row in rows {
        let (_id, ack_path) = row?;
        if ack_path
            .as_deref()
            .is_none_or(|candidate| paths_overlap(candidate, path))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Load directed acknowledgement and resolve updates for an agent.
pub(crate) fn directed_typed_updates(
    conn: &Connection,
    agent_id: &str,
) -> anyhow::Result<Vec<UnreadUpdate>> {
    let mut updates = Vec::new();

    let mut ack_stmt = conn.prepare(
        "SELECT id, created_at_ms, agent_id, note
         FROM acknowledgements
         WHERE target_agent_id = ?1 AND agent_id <> ?1
         ORDER BY id ASC
         LIMIT 20",
    )?;
    let ack_rows = ack_stmt.query_map(params![agent_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in ack_rows {
        let (id, created_at_ms, from_agent, note) = row?;
        let paths = load_ack_paths(conn, id)?;
        updates.push(UnreadUpdate {
            id: 1_000_000_000_000i64.saturating_add(id),
            created_at_ms,
            agent_id: from_agent,
            kind: "ack".to_owned(),
            body: json!({
                "ack_id": id,
                "target_agent_id": agent_id,
                "note": note,
                "paths": paths,
            }),
        });
    }

    let mut resolve_stmt = conn.prepare(
        "SELECT id, resolved_at_ms, resolved_by_agent_id
         FROM blocks
         WHERE agent_id = ?1
           AND resolved_at_ms IS NOT NULL
           AND resolved_by_agent_id IS NOT NULL
           AND resolved_by_agent_id <> ?1
         ORDER BY resolved_at_ms ASC
         LIMIT 20",
    )?;
    let resolve_rows = resolve_stmt.query_map(params![agent_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in resolve_rows {
        let (block_id, created_at_ms, resolved_by_agent_id) = row?;
        let paths = load_block_paths(conn, block_id)?;
        updates.push(UnreadUpdate {
            id: 2_000_000_000_000i64.saturating_add(block_id),
            created_at_ms,
            agent_id: resolved_by_agent_id.clone(),
            kind: "resolve".to_owned(),
            body: json!({
                "block_id": block_id,
                "target_agent_id": agent_id,
                "resolved_by_agent_id": resolved_by_agent_id,
                "paths": paths,
            }),
        });
    }

    updates.sort_by_key(|update| (update.created_at_ms, update.id));
    Ok(updates)
}

/// Group joined block rows into block objects.
fn group_blocks(rows: Vec<BlockRow>) -> Vec<TypedBlock> {
    let mut grouped = BTreeMap::<i64, TypedBlock>::new();
    for (
        id,
        agent_id,
        mode,
        reason,
        created_at_ms,
        expires_at_ms,
        resolved_at_ms,
        resolved_by_agent_id,
        path,
    ) in rows
    {
        let entry = grouped.entry(id).or_insert_with(|| TypedBlock {
            id,
            agent_id,
            mode,
            reason,
            paths: Vec::new(),
            created_at_ms,
            expires_at_ms,
            resolved_at_ms,
            resolved_by_agent_id,
        });
        entry.paths.push(path);
    }
    grouped.into_values().collect()
}

/// Load ordered paths for one acknowledgement.
fn load_ack_paths(conn: &Connection, ack_id: i64) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT path
         FROM ack_paths
         WHERE ack_id = ?1
         ORDER BY path ASC",
    )?;
    let rows = stmt.query_map(params![ack_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Load ordered paths for one block.
fn load_block_paths(conn: &Connection, block_id: i64) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT path
         FROM block_paths
         WHERE block_id = ?1
         ORDER BY path ASC",
    )?;
    let rows = stmt.query_map(params![block_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Query open blocks using either a connection or a transaction.
fn load_open_blocks_on(
    conn: &impl PrepareSql,
    except_agent_id: Option<&str>,
    overlap_path: Option<&str>,
    now_ms: i64,
) -> anyhow::Result<Vec<TypedBlock>> {
    let rows = match (except_agent_id, overlap_path) {
        (Some(agent_id), Some(path)) => {
            let mut stmt = conn.prepare_query(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.agent_id <> ?1
                   AND b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?2)
                   AND (bp.path = ?3
                        OR substr(?3, 1, length(bp.path) + 1) = bp.path || '/'
                        OR substr(bp.path, 1, length(?3) + 1) = ?3 || '/')
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            let iter = stmt.query_map(params![agent_id, now_ms, path], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?;
            iter.collect::<rusqlite::Result<Vec<_>>>()?
        }
        (Some(agent_id), None) => {
            let mut stmt = conn.prepare_query(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.agent_id <> ?1
                   AND b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?2)
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            let iter = stmt.query_map(params![agent_id, now_ms], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?;
            iter.collect::<rusqlite::Result<Vec<_>>>()?
        }
        (None, Some(path)) => {
            let mut stmt = conn.prepare_query(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?1)
                   AND (bp.path = ?2
                        OR substr(?2, 1, length(bp.path) + 1) = bp.path || '/'
                        OR substr(bp.path, 1, length(?2) + 1) = ?2 || '/')
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            let iter = stmt.query_map(params![now_ms, path], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?;
            iter.collect::<rusqlite::Result<Vec<_>>>()?
        }
        (None, None) => {
            let mut stmt = conn.prepare_query(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?1)
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            let iter = stmt.query_map(params![now_ms], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?;
            iter.collect::<rusqlite::Result<Vec<_>>>()?
        }
    };

    Ok(group_blocks(rows))
}
