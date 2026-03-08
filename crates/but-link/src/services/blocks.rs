//! Service handlers for typed block, resolve, and ack commands.

use std::time::Duration;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::cli::BlockMode;
use crate::db;
use crate::repo;

/// Response payload for `block`.
#[derive(Debug, Serialize)]
pub(crate) struct BlockCreated {
    /// Standard success marker.
    pub ok: bool,
    /// Created block identifier.
    pub block_id: i64,
}

/// Response payload for `resolve`.
#[derive(Debug, Serialize)]
pub(crate) struct BlockResolved {
    /// Standard success marker.
    pub ok: bool,
    /// Resolved block identifier.
    pub resolved_block_id: i64,
}

/// Response payload for `ack`.
#[derive(Debug, Serialize)]
pub(crate) struct AckCreated {
    /// Standard success marker.
    pub ok: bool,
    /// Created acknowledgement identifier.
    pub ack_id: i64,
}

/// Create a typed authoritative block.
pub(crate) fn block(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    reason: &str,
    mode: BlockMode,
    ttl: Option<Duration>,
) -> anyhow::Result<BlockCreated> {
    let now_ms = db::now_unix_ms()?;
    let normalized = repo::normalized_unique_paths(paths);
    let expires_at_ms = ttl
        .map(|duration| i64::try_from(duration.as_millis()))
        .transpose()?
        .map(|millis| now_ms.saturating_add(millis));
    let mode_str = match mode {
        BlockMode::Advisory => "advisory",
        BlockMode::Hard => "hard",
    };

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL)",
        params![agent_id, mode_str, reason, now_ms, expires_at_ms],
    )?;
    let block_id = tx.last_insert_rowid();
    for path in &normalized {
        tx.execute(
            "INSERT INTO block_paths(block_id, path) VALUES (?1, ?2)",
            params![block_id, path],
        )?;
    }
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    Ok(BlockCreated { ok: true, block_id })
}

/// Resolve a typed authoritative block.
pub(crate) fn resolve(
    conn: &Connection,
    agent_id: &str,
    block_id: i64,
) -> anyhow::Result<BlockResolved> {
    let now_ms = db::now_unix_ms()?;
    let updated = conn.execute(
        "UPDATE blocks
         SET resolved_at_ms = ?2, resolved_by_agent_id = ?3
         WHERE id = ?1 AND resolved_at_ms IS NULL",
        params![block_id, now_ms, agent_id],
    )?;
    anyhow::ensure!(
        updated > 0,
        "block not found or already resolved: {block_id}"
    );
    db::touch_agent_progress_at(conn, agent_id, now_ms)?;
    Ok(BlockResolved {
        ok: true,
        resolved_block_id: block_id,
    })
}

/// Record an authoritative acknowledgement.
pub(crate) fn ack(
    conn: &mut Connection,
    agent_id: &str,
    target_agent_id: &str,
    paths: &[String],
    note: Option<&str>,
) -> anyhow::Result<AckCreated> {
    let now_ms = db::now_unix_ms()?;
    let normalized = repo::normalized_unique_paths(paths);
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO acknowledgements(agent_id, target_agent_id, note, created_at_ms)
         VALUES (?1, ?2, ?3, ?4)",
        params![agent_id, target_agent_id, note, now_ms],
    )?;
    let ack_id = tx.last_insert_rowid();
    for path in &normalized {
        tx.execute(
            "INSERT INTO ack_paths(ack_id, path) VALUES (?1, ?2)",
            params![ack_id, path],
        )?;
    }
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    Ok(AckCreated { ok: true, ack_id })
}
