//! Database initialization, migrations, and typed coordination queries.
//!
//! All database access shared across command handlers and the TUI lives here.

use std::collections::{BTreeMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::payloads::{DiscoveryPayload, SurfacePayload};
use crate::text::{
    contains_path_token, extract_message_text, parse_body, relevant_needles_for_path,
};

/// Agent snapshot used by commands and the TUI.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AgentSnapshot {
    /// Stable agent identifier.
    pub agent_id: String,
    /// Optional short status string.
    pub status: Option<String>,
    /// Optional short plan string.
    pub plan: Option<String>,
    /// Legacy compatibility timestamp seeded from progress updates.
    pub updated_at_ms: i64,
    /// Last observed command from the agent.
    pub last_seen_at_ms: i64,
    /// Last command that counts as progress.
    pub last_progress_at_ms: i64,
}

/// Active claim row used in structured responses.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ActiveClaim {
    /// Claimed repo-relative path.
    pub path: String,
    /// Owning agent identifier.
    pub agent_id: String,
    /// Claim expiry in unix milliseconds.
    pub expires_at_ms: i64,
}

/// Typed coordination block used in structured responses.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct TypedBlock {
    /// Block identifier.
    pub id: i64,
    /// Agent that created the block.
    pub agent_id: String,
    /// `hard` or `advisory`.
    pub mode: String,
    /// Human-readable reason.
    pub reason: String,
    /// Covered repo-relative paths.
    pub paths: Vec<String>,
    /// Creation timestamp in unix milliseconds.
    pub created_at_ms: i64,
    /// Optional expiry timestamp in unix milliseconds.
    pub expires_at_ms: Option<i64>,
    /// Optional resolution timestamp in unix milliseconds.
    pub resolved_at_ms: Option<i64>,
    /// Optional resolving agent.
    pub resolved_by_agent_id: Option<String>,
}

/// Dependency hint emitted when intents and declarations overlap.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DependencyHint {
    /// Stable kind tag for machine consumers.
    pub kind: &'static str,
    /// Agent that declared the dependency surface.
    pub provider_agent_id: String,
    /// Shared scope of the surface.
    pub scope: String,
    /// Tags attached to the declaration.
    pub tags: Vec<String>,
    /// Overlapping surface tokens.
    pub overlap_tokens: Vec<String>,
    /// Optional overlapping scoped paths.
    pub overlap_paths: Vec<String>,
    /// Human-readable explanation.
    pub why: String,
}

/// Stale claim holder summary.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct StaleAgent {
    /// Stable kind tag for machine consumers.
    pub kind: &'static str,
    /// Agent id of the stale holder.
    pub agent_id: String,
    /// Last progress timestamp.
    pub last_progress_at_ms: i64,
    /// How long the holder has been stale.
    pub stale_for_ms: i64,
    /// Configured stale threshold.
    pub threshold_ms: i64,
    /// Whether the holder is stale.
    pub is_stale: bool,
    /// Relevant claimed paths owned by the stale agent.
    pub claim_paths: Vec<String>,
}

/// Unread inbox entry surfaced by `read`.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct UnreadUpdate {
    /// Message id cursor.
    pub id: i64,
    /// Creation timestamp.
    pub created_at_ms: i64,
    /// Sender agent.
    pub agent_id: String,
    /// Message kind.
    pub kind: String,
    /// Parsed body payload.
    pub body: Value,
}

/// Claim detail selected for a requester.
#[derive(Clone, Debug)]
pub(crate) struct SelfClaimState {
    /// `active` or `stale`.
    pub status: &'static str,
    /// Matching claimed path.
    pub path: String,
    /// Claim expiry in unix milliseconds.
    pub expires_at_ms: i64,
}

/// Joined block row used when grouping block query results in this module.
type BlockRow = (
    i64,
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<i64>,
    Option<String>,
    String,
);

/// Shared statement preparation across connections and transactions.
pub(crate) trait PrepareSql {
    /// Prepare an SQL statement on the underlying SQLite handle.
    fn prepare_query<'a>(&'a self, sql: &str) -> rusqlite::Result<rusqlite::Statement<'a>>;
}

impl PrepareSql for Connection {
    fn prepare_query<'a>(&'a self, sql: &str) -> rusqlite::Result<rusqlite::Statement<'a>> {
        self.prepare(sql)
    }
}

impl PrepareSql for Transaction<'_> {
    fn prepare_query<'a>(&'a self, sql: &str) -> rusqlite::Result<rusqlite::Statement<'a>> {
        self.prepare(sql)
    }
}

/// Initialize and migrate the coordination database.
pub(crate) fn init_db(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA busy_timeout = 5000;",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS claims (\
            path TEXT NOT NULL,\
            agent_id TEXT NOT NULL,\
            expires_at_ms INTEGER NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_claims_path_expires \
            ON claims(path, expires_at_ms);\
         CREATE TABLE IF NOT EXISTS agent_state (\
            agent_id TEXT PRIMARY KEY,\
            status TEXT,\
            plan TEXT,\
            updated_at_ms INTEGER NOT NULL DEFAULT 0,\
            last_seen_at_ms INTEGER NOT NULL DEFAULT 0,\
            last_progress_at_ms INTEGER NOT NULL DEFAULT 0\
         );\
         CREATE TABLE IF NOT EXISTS messages (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            created_at_ms INTEGER NOT NULL,\
            agent_id TEXT NOT NULL,\
            kind TEXT NOT NULL,\
            body_json TEXT NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS agent_cursors (\
            agent_id TEXT NOT NULL,\
            topic TEXT NOT NULL,\
            last_seen_msg_id INTEGER NOT NULL,\
            updated_at_ms INTEGER NOT NULL,\
            PRIMARY KEY(agent_id, topic)\
         );\
         CREATE TABLE IF NOT EXISTS blocks (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            agent_id TEXT NOT NULL,\
            mode TEXT NOT NULL,\
            reason TEXT NOT NULL,\
            created_at_ms INTEGER NOT NULL,\
            expires_at_ms INTEGER,\
            resolved_at_ms INTEGER,\
            resolved_by_agent_id TEXT\
         );\
         CREATE TABLE IF NOT EXISTS block_paths (\
            block_id INTEGER NOT NULL,\
            path TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_block_paths_path ON block_paths(path);\
         CREATE INDEX IF NOT EXISTS idx_blocks_active ON blocks(resolved_at_ms, expires_at_ms, created_at_ms);\
         CREATE TABLE IF NOT EXISTS acknowledgements (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            agent_id TEXT NOT NULL,\
            target_agent_id TEXT NOT NULL,\
            note TEXT,\
            created_at_ms INTEGER NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS ack_paths (\
            ack_id INTEGER NOT NULL,\
            path TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_ack_paths_path ON ack_paths(path);",
    )
    .context("init_db")?;

    migrate_agent_state_columns(conn)?;
    prune(conn)?;
    Ok(())
}

/// Maximum number of transcript messages retained in the database.
const MAX_MESSAGES: i64 = 10_000;

/// Ensure agent state has the split seen/progress columns.
fn migrate_agent_state_columns(conn: &Connection) -> anyhow::Result<()> {
    let columns = table_columns(conn, "agent_state")?;
    if !columns.contains("last_seen_at_ms") {
        conn.execute(
            "ALTER TABLE agent_state ADD COLUMN last_seen_at_ms INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !columns.contains("last_progress_at_ms") {
        conn.execute(
            "ALTER TABLE agent_state ADD COLUMN last_progress_at_ms INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    conn.execute(
        "UPDATE agent_state
         SET last_seen_at_ms = CASE
                 WHEN last_seen_at_ms = 0 THEN updated_at_ms
                 ELSE last_seen_at_ms
             END,
             last_progress_at_ms = CASE
                 WHEN last_progress_at_ms = 0 THEN updated_at_ms
                 ELSE last_progress_at_ms
             END",
        [],
    )?;
    Ok(())
}

/// Look up table column names with `PRAGMA table_info`.
fn table_columns(conn: &Connection, table_name: &str) -> anyhow::Result<HashSet<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .collect())
}

/// Prune expired claims and cap message growth.
fn prune(conn: &Connection) -> anyhow::Result<()> {
    let now_ms = now_unix_ms()?;
    conn.execute(
        "DELETE FROM claims WHERE expires_at_ms <= ?1",
        params![now_ms],
    )?;

    let count: i64 = conn.query_row("SELECT COUNT(1) FROM messages", [], |row| row.get(0))?;
    if count > MAX_MESSAGES {
        conn.execute(
            "DELETE FROM messages WHERE id NOT IN (
                SELECT id FROM messages ORDER BY id DESC LIMIT ?1
             )",
            params![MAX_MESSAGES],
        )?;
    }
    Ok(())
}

/// Ensure an agent row exists.
pub(crate) fn ensure_agent_row(
    conn: &Connection,
    agent_id: &str,
    _now_ms: i64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO agent_state(
            agent_id,
            status,
            plan,
            updated_at_ms,
            last_seen_at_ms,
            last_progress_at_ms
         ) VALUES (?1, NULL, NULL, 0, 0, 0)
         ON CONFLICT(agent_id) DO NOTHING",
        params![agent_id],
    )
    .context("ensure_agent_row")?;
    Ok(())
}

/// Record that an agent was seen executing any command.
pub(crate) fn touch_agent_seen(conn: &Connection, agent_id: &str) -> anyhow::Result<()> {
    let now_ms = now_unix_ms()?;
    ensure_agent_row(conn, agent_id, now_ms)?;
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
    ensure_agent_row(conn, agent_id, now_ms)?;
    conn.execute(
        "UPDATE agent_state
         SET updated_at_ms = ?2,
             last_seen_at_ms = ?2,
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
            updated_at_ms,
            last_seen_at_ms,
            last_progress_at_ms
         ) VALUES (?1, NULL, NULL, 0, 0, 0)
         ON CONFLICT(agent_id) DO NOTHING",
        params![agent_id],
    )?;
    tx.execute(
        "UPDATE agent_state
         SET updated_at_ms = ?2,
             last_seen_at_ms = ?2,
             last_progress_at_ms = ?2
         WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;
    Ok(())
}

/// Persist a transcript message.
pub(crate) fn insert_history_message(
    conn: &Connection,
    created_at_ms: i64,
    agent_id: &str,
    kind: &str,
    body_json: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
        params![created_at_ms, agent_id, kind, body_json],
    )?;
    Ok(())
}

/// Persist a transcript message inside an existing transaction.
pub(crate) fn insert_history_message_tx(
    tx: &Transaction<'_>,
    created_at_ms: i64,
    agent_id: &str,
    kind: &str,
    body_json: &str,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
        params![created_at_ms, agent_id, kind, body_json],
    )?;
    Ok(())
}

/// Load the owner of a typed block when it exists.
pub(crate) fn load_block_owner(conn: &Connection, block_id: i64) -> anyhow::Result<Option<String>> {
    conn.query_row(
        "SELECT agent_id FROM blocks WHERE id = ?1",
        params![block_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

/// Return the current unix timestamp in milliseconds.
pub(crate) fn now_unix_ms() -> anyhow::Result<i64> {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock")?;
    dur.as_millis().try_into().context("timestamp overflow")
}

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

/// Determine the requester's current claim state for a path.
pub(crate) fn load_self_claim_state(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Option<SelfClaimState>> {
    let active = conn
        .query_row(
            "SELECT path, expires_at_ms FROM claims
             WHERE agent_id = ?1 AND expires_at_ms > ?2
               AND (path = ?3
                    OR substr(?3, 1, length(path) + 1) = path || '/'
                    OR substr(path, 1, length(?3) + 1) = ?3 || '/')
             ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC
             LIMIT 1",
            params![agent_id, now_ms, path],
            |row| {
                Ok(SelfClaimState {
                    status: "active",
                    path: row.get(0)?,
                    expires_at_ms: row.get(1)?,
                })
            },
        )
        .optional()?;
    if active.is_some() {
        return Ok(active);
    }

    conn.query_row(
        "SELECT path, expires_at_ms FROM claims
         WHERE agent_id = ?1 AND expires_at_ms <= ?2
           AND (path = ?3
                OR substr(?3, 1, length(path) + 1) = path || '/'
                OR substr(path, 1, length(?3) + 1) = ?3 || '/')
         ORDER BY expires_at_ms DESC, LENGTH(path) DESC, path ASC
         LIMIT 1",
        params![agent_id, now_ms, path],
        |row| {
            Ok(SelfClaimState {
                status: "stale",
                path: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// Query claim conflicts for a single path.
pub(crate) fn claim_conflicts(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<ActiveClaim>> {
    claim_conflicts_on(conn, agent_id, path, now_ms)
}

/// Query claim conflicts for a single path inside a transaction.
pub(crate) fn claim_conflicts_tx(
    tx: &Transaction<'_>,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<ActiveClaim>> {
    claim_conflicts_on(tx, agent_id, path, now_ms)
}

/// Load agent state snapshots ordered by recent progress.
pub(crate) fn load_agent_snapshots(conn: &Connection) -> anyhow::Result<Vec<AgentSnapshot>> {
    let mut stmt = conn.prepare(
        "SELECT agent_id, status, plan, updated_at_ms, last_seen_at_ms, last_progress_at_ms
         FROM agent_state
         ORDER BY last_progress_at_ms DESC, agent_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AgentSnapshot {
            agent_id: row.get(0)?,
            status: row.get(1)?,
            plan: row.get(2)?,
            updated_at_ms: row.get(3)?,
            last_seen_at_ms: row.get(4)?,
            last_progress_at_ms: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Load open typed blocks, optionally filtered to overlaps with a path.
pub(crate) fn load_open_blocks(
    conn: &Connection,
    except_agent_id: Option<&str>,
    overlap_path: Option<&str>,
) -> anyhow::Result<Vec<TypedBlock>> {
    load_open_blocks_on(conn, except_agent_id, overlap_path, now_unix_ms()?)
}

/// Load open typed blocks inside an existing transaction.
pub(crate) fn load_open_blocks_tx(
    tx: &Transaction<'_>,
    except_agent_id: Option<&str>,
    overlap_path: Option<&str>,
    now_ms: i64,
) -> anyhow::Result<Vec<TypedBlock>> {
    load_open_blocks_on(tx, except_agent_id, overlap_path, now_ms)
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

/// Load unread inbox updates for an agent and advance the cursor.
pub(crate) fn unread_inbox_updates(
    conn: &Connection,
    agent_id: &str,
    now_ms: i64,
) -> anyhow::Result<(Vec<UnreadUpdate>, i64, i64)> {
    let topic = "inbox";
    let prev_cursor: i64 = conn
        .query_row(
            "SELECT last_seen_msg_id FROM agent_cursors WHERE agent_id = ?1 AND topic = ?2",
            params![agent_id, topic],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);

    let mention = format!("@{agent_id}");
    let mut stmt = conn.prepare(
        "SELECT id, created_at_ms, agent_id, kind, body_json
         FROM messages
         WHERE id > ?1 AND agent_id <> ?2
         ORDER BY id ASC
         LIMIT 500",
    )?;
    let rows = stmt.query_map(params![prev_cursor, agent_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut updates = Vec::new();
    let mut max_id = prev_cursor;
    let mut last_returned_id = prev_cursor;
    let mut reached_limit = false;
    for row in rows {
        let (id, created_at_ms, from_agent, kind, body_json) = row?;
        max_id = max_id.max(id);
        let body: Value =
            serde_json::from_str(&body_json).unwrap_or(Value::String(body_json.clone()));
        let text = extract_message_text(&body, &body_json);
        let is_directed = text.contains(&mention)
            || body
                .get("target_agent_id")
                .and_then(Value::as_str)
                .is_some_and(|target| target == agent_id);
        if !is_directed {
            continue;
        }
        if updates.len() >= 20 {
            reached_limit = true;
            continue;
        }
        last_returned_id = id;
        updates.push(UnreadUpdate {
            id,
            created_at_ms,
            agent_id: from_agent,
            kind,
            body,
        });
    }

    let new_cursor = if reached_limit {
        last_returned_id
    } else {
        max_id
    };
    if new_cursor != prev_cursor {
        conn.execute(
            "INSERT INTO agent_cursors(agent_id, topic, last_seen_msg_id, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(agent_id, topic)
             DO UPDATE SET last_seen_msg_id = excluded.last_seen_msg_id, updated_at_ms = excluded.updated_at_ms",
            params![agent_id, topic, new_cursor, now_ms],
        )?;
    }
    Ok((updates, prev_cursor, new_cursor))
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
        if !paths.is_empty() && !paths.iter().any(|path| paths_overlap(path, &claim.path)) {
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

/// Compute dependency hints scoped to the requested paths.
pub(crate) fn dependency_hints_for_paths(
    conn: &Connection,
    agent_id: &str,
    requested_paths: &[String],
) -> anyhow::Result<Vec<DependencyHint>> {
    if requested_paths.is_empty() {
        return Ok(Vec::new());
    }

    let mut intent_stmt = conn.prepare(
        "SELECT body_json FROM messages
         WHERE kind = 'intent' AND agent_id = ?1
         ORDER BY id DESC
         LIMIT 50",
    )?;
    let intents = intent_stmt
        .query_map(params![agent_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if intents.is_empty() {
        return Ok(Vec::new());
    }

    let requester_intents: Vec<SurfacePayload> = intents
        .into_iter()
        .filter_map(|body| SurfacePayload::from_json_str(&body).ok())
        .filter(|intent| {
            intent.paths.is_empty()
                || intent.paths.iter().any(|intent_path| {
                    requested_paths
                        .iter()
                        .any(|req| paths_overlap(intent_path, req))
                })
        })
        .collect();
    if requester_intents.is_empty() {
        return Ok(Vec::new());
    }

    let mut decl_stmt = conn.prepare(
        "SELECT agent_id, body_json FROM messages
         WHERE kind = 'declaration' AND agent_id <> ?1
         ORDER BY id DESC",
    )?;
    let declarations = decl_stmt.query_map(params![agent_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut hints = Vec::new();
    let mut seen_provider_scope = HashSet::<(String, String)>::new();
    for row in declarations {
        let (provider_agent_id, body_json) = row?;
        let declaration = match SurfacePayload::from_json_str(&body_json) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        if !declaration.tags.iter().any(|tag| tag_has_api_segment(tag)) {
            continue;
        }

        for intent in &requester_intents {
            if !intent.scope.is_empty()
                && !declaration.scope.is_empty()
                && intent.scope != declaration.scope
            {
                continue;
            }
            let overlap_tokens: Vec<String> = declaration
                .surface
                .iter()
                .filter(|token| intent.surface.iter().any(|own| own == *token))
                .cloned()
                .collect();
            if overlap_tokens.is_empty() {
                continue;
            }

            let overlap_paths =
                scoped_overlap_paths(&intent.paths, &declaration.paths, requested_paths);
            let path_match = if intent.paths.is_empty() && declaration.paths.is_empty() {
                true
            } else {
                !overlap_paths.is_empty()
            };
            if !path_match {
                continue;
            }

            if !seen_provider_scope.insert((provider_agent_id.clone(), declaration.scope.clone())) {
                continue;
            }

            let why = if overlap_paths.is_empty() {
                format!(
                    "intent/declaration overlap on token(s): {} within scope {}",
                    overlap_tokens.join(", "),
                    declaration.scope
                )
            } else {
                format!(
                    "intent/declaration overlap on token(s): {} for path(s): {}",
                    overlap_tokens.join(", "),
                    overlap_paths.join(", ")
                )
            };
            hints.push(DependencyHint {
                kind: "dependency_hint",
                provider_agent_id: provider_agent_id.clone(),
                scope: declaration.scope.clone(),
                tags: declaration.tags.clone(),
                overlap_tokens,
                overlap_paths,
                why,
            });
        }
    }

    Ok(hints)
}

/// Load transcript messages since a timestamp.
pub(crate) fn load_messages_since(
    conn: &Connection,
    kind: Option<&str>,
    since_ms: i64,
) -> anyhow::Result<Vec<Value>> {
    let mut messages = Vec::new();
    let mut push_message = |created_at_ms: i64, agent: String, kind: String, body_json: String| {
        let (body_v, content) = parse_body(&body_json);
        messages.push(json!({
            "created_at_ms": created_at_ms,
            "agent_id": agent,
            "kind": kind,
            "body": body_v,
            "content": content,
        }));
    };

    let kind = kind.filter(|kind| !kind.is_empty());
    if let Some(kind_filter) = kind
        && kind_filter != "all"
    {
        let mut stmt = conn.prepare(
            "SELECT created_at_ms, agent_id, kind, body_json FROM messages
             WHERE created_at_ms >= ?1 AND kind = ?2
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![since_ms, kind_filter], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (created_at_ms, agent, kind, body_json) = row?;
            push_message(created_at_ms, agent, kind, body_json);
        }
        return Ok(messages);
    }

    let mut stmt = conn.prepare(
        "SELECT created_at_ms, agent_id, kind, body_json FROM messages
         WHERE created_at_ms >= ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![since_ms], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (created_at_ms, agent, kind, body_json) = row?;
        push_message(created_at_ms, agent, kind, body_json);
    }
    Ok(messages)
}

/// Load discovery messages with optional high-signal filtering.
pub(crate) fn load_discoveries_and_next_steps(
    conn: &Connection,
    kind: &str,
    all: bool,
) -> anyhow::Result<(Vec<Value>, Vec<Value>)> {
    let mut discoveries = Vec::new();
    let mut next_steps = Vec::new();

    if kind == "discovery" || kind == "all" {
        let mut stmt = conn.prepare(
            "SELECT agent_id, body_json FROM messages WHERE kind = 'discovery' ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (agent, body_json) = row?;
            match DiscoveryPayload::from_json_str(&body_json) {
                Ok(payload) => {
                    if !all && !payload.is_high_signal() {
                        continue;
                    }
                    if let Some(cmd) = payload.command() {
                        next_steps.push(json!({ "kind": "run", "cmd": cmd }));
                    }
                    discoveries.push(payload.to_value_with_metadata(&agent, None, false)?);
                }
                Err(_) => {
                    let (mut obj, _) = parse_body(&body_json);
                    if let Value::Object(map) = &mut obj {
                        map.insert("agent_id".to_owned(), Value::String(agent));
                    }
                    if !all && !is_high_signal_discovery(&obj) {
                        continue;
                    }
                    if let Some(cmd) = obj
                        .get("suggested_action")
                        .and_then(|value| value.get("cmd"))
                        .and_then(Value::as_str)
                    {
                        next_steps.push(json!({ "kind": "run", "cmd": cmd }));
                    }
                    discoveries.push(obj);
                }
            }
        }
    }

    Ok((discoveries, next_steps))
}

/// Validate discovery payloads accepted by `post --type discovery`.
pub(crate) fn validate_discovery_payload(v: &Value) -> anyhow::Result<()> {
    DiscoveryPayload::from_value(v.clone())?.validate()
}

/// Validate typed surface payloads accepted by `post --type intent|declaration`.
pub(crate) fn validate_surface_payload(v: &Value) -> anyhow::Result<()> {
    SurfacePayload::from_value(v.clone())?.validate()
}

/// Return path overlap between scoped intent/declaration data and requested paths.
fn scoped_overlap_paths(
    intent_paths: &[String],
    declaration_paths: &[String],
    requested_paths: &[String],
) -> Vec<String> {
    if intent_paths.is_empty() && declaration_paths.is_empty() {
        return Vec::new();
    }

    let mut overlap = Vec::new();
    if !intent_paths.is_empty() && !declaration_paths.is_empty() {
        for intent_path in intent_paths {
            if declaration_paths
                .iter()
                .any(|decl_path| paths_overlap(intent_path, decl_path))
            {
                overlap.push(intent_path.clone());
            }
        }
        overlap.sort();
        overlap.dedup();
        return overlap;
    }

    let scoped = if intent_paths.is_empty() {
        declaration_paths
    } else {
        intent_paths
    };
    for path in scoped {
        if requested_paths
            .iter()
            .any(|requested| paths_overlap(path, requested))
        {
            overlap.push(path.clone());
        }
    }
    overlap.sort();
    overlap.dedup();
    overlap
}

/// Check if a tag contains an `api` segment.
fn tag_has_api_segment(tag: &str) -> bool {
    tag.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|segment| segment.eq_ignore_ascii_case("api"))
}

/// Check whether two repo-relative paths overlap literally.
pub(crate) fn paths_overlap(lhs: &str, rhs: &str) -> bool {
    lhs == rhs || lhs.starts_with(&format!("{rhs}/")) || rhs.starts_with(&format!("{lhs}/"))
}

/// Determine whether a discovery is high signal.
fn is_high_signal_discovery(value: &Value) -> bool {
    value
        .get("signal")
        .and_then(Value::as_str)
        .is_some_and(|signal| signal.eq_ignore_ascii_case("high"))
}

/// Resolve the stale coordination threshold.
pub(crate) fn coord_stale_threshold_ms() -> i64 {
    let default_s: i64 = 15 * 60;
    let seconds = std::env::var("COORD_STALE_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 0)
        .unwrap_or(default_s);
    seconds.saturating_mul(1000)
}

/// Load recent discovery messages that mention any requested path.
pub(crate) fn recent_discoveries_for_paths(
    conn: &impl PrepareSql,
    paths: &[String],
    limit: usize,
) -> anyhow::Result<Vec<Value>> {
    if paths.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let needles: Vec<String> = paths
        .iter()
        .flat_map(|path| relevant_needles_for_path(path))
        .collect();
    let mut stmt = conn.prepare_query(
        "SELECT created_at_ms, agent_id, body_json FROM messages
         WHERE kind = 'discovery'
         ORDER BY id DESC
         LIMIT 100",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut discoveries = Vec::new();
    for row in rows {
        let (created_at_ms, agent_id, body_json) = row?;
        let (mut body, text) = parse_body(&body_json);
        if !needles
            .iter()
            .any(|needle| contains_path_token(&text, needle))
        {
            continue;
        }
        if let Ok(payload) = DiscoveryPayload::from_json_str(&body_json) {
            discoveries.push(payload.to_value_with_metadata(
                &agent_id,
                Some(created_at_ms),
                true,
            )?);
        } else {
            if let Value::Object(map) = &mut body {
                map.insert("created_at_ms".to_owned(), Value::from(created_at_ms));
                map.insert("agent_id".to_owned(), Value::String(agent_id));
                map.insert("kind".to_owned(), Value::String("discovery".to_owned()));
            }
            discoveries.push(body);
        }
        if discoveries.len() >= limit {
            break;
        }
    }
    Ok(discoveries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Insert a message row used by cursor and dependency-hint tests.
    fn insert_message(
        conn: &Connection,
        created_at_ms: i64,
        agent_id: &str,
        kind: &str,
        body: Value,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
            params![created_at_ms, agent_id, kind, body.to_string()],
        )?;
        Ok(())
    }

    #[test]
    fn init_db_migrates_agent_state_columns() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE agent_state (
                agent_id TEXT PRIMARY KEY,
                status TEXT,
                plan TEXT,
                updated_at_ms INTEGER NOT NULL
            );",
        )?;
        conn.execute(
            "INSERT INTO agent_state(agent_id, status, plan, updated_at_ms) VALUES ('a', NULL, NULL, 42)",
            [],
        )?;

        init_db(&conn)?;

        let snapshot = load_agent_snapshots(&conn)?.remove(0);
        assert_eq!(snapshot.last_seen_at_ms, 42);
        assert_eq!(snapshot.last_progress_at_ms, 42);
        Ok(())
    }

    #[test]
    fn unread_inbox_updates_track_directed_messages() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        init_db(&conn)?;
        insert_message(
            &conn,
            1,
            "peer",
            "message",
            json!({ "text": "@me: ack: saw it" }),
        )?;
        insert_message(&conn, 2, "peer", "message", json!({ "text": "unrelated" }))?;

        let (updates, prev_cursor, new_cursor) = unread_inbox_updates(&conn, "me", 1_000)?;
        assert_eq!(prev_cursor, 0);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].agent_id, "peer");
        assert_eq!(new_cursor, 2);
        Ok(())
    }

    #[test]
    fn dependency_hints_filter_by_requested_paths() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        init_db(&conn)?;
        insert_message(
            &conn,
            1,
            "me",
            "intent",
            json!({
                "scope": "crate::auth",
                "tags": ["api"],
                "surface": ["AuthToken"],
                "paths": ["src/auth.rs"]
            }),
        )?;
        insert_message(
            &conn,
            2,
            "peer",
            "declaration",
            json!({
                "scope": "crate::auth",
                "tags": ["api"],
                "surface": ["AuthToken"],
                "paths": ["src/auth.rs"]
            }),
        )?;
        insert_message(
            &conn,
            3,
            "peer-b",
            "declaration",
            json!({
                "scope": "crate::auth",
                "tags": ["api"],
                "surface": ["AuthToken"],
                "paths": ["src/other.rs"]
            }),
        )?;

        let hints = dependency_hints_for_paths(&conn, "me", &[String::from("src/auth.rs")])?;
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].provider_agent_id, "peer");
        Ok(())
    }

    #[test]
    fn ack_exists_since_matches_scoped_and_generic_acks() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        init_db(&conn)?;
        conn.execute(
            "INSERT INTO acknowledgements(agent_id, target_agent_id, note, created_at_ms)
             VALUES ('me', 'peer', NULL, 10)",
            [],
        )?;
        let generic_ack_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO acknowledgements(agent_id, target_agent_id, note, created_at_ms)
             VALUES ('me', 'peer', NULL, 11)",
            [],
        )?;
        let scoped_ack_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO ack_paths(ack_id, path) VALUES (?1, 'src/app.txt')",
            params![scoped_ack_id],
        )?;

        assert!(ack_exists_since(&conn, "me", "peer", "src/other.txt", 0)?);
        conn.execute(
            "DELETE FROM acknowledgements WHERE id = ?1",
            params![generic_ack_id],
        )?;
        assert!(ack_exists_since(&conn, "me", "peer", "src/app.txt", 0)?);
        assert!(!ack_exists_since(&conn, "me", "peer", "src/other.txt", 0)?);
        Ok(())
    }

    #[test]
    fn load_open_blocks_matches_between_connection_and_transaction() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        init_db(&conn)?;
        conn.execute(
            "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
             VALUES ('peer', 'hard', 'shared refactor', 10, NULL, NULL, NULL)",
            [],
        )?;
        let block_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO block_paths(block_id, path) VALUES (?1, 'src/app.rs')",
            params![block_id],
        )?;

        let expected = load_open_blocks(&conn, Some("me"), Some("src/app.rs"))?;
        let tx = conn.transaction()?;
        let actual = load_open_blocks_tx(&tx, Some("me"), Some("src/app.rs"), now_unix_ms()?)?;

        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual[0].id, expected[0].id);
        assert_eq!(actual[0].paths, expected[0].paths);
        Ok(())
    }
}
