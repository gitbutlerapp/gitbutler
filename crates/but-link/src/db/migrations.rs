//! Schema initialization and migrations for the coordination database.

use std::collections::HashSet;

use anyhow::Context;
use rusqlite::{Connection, params};

use super::now_unix_ms;

/// Maximum number of transcript messages retained in the database.
const MAX_MESSAGES: i64 = 10_000;

/// Initialize and migrate the coordination database.
pub(crate) fn init_db(conn: &Connection) -> anyhow::Result<()> {
    but_db::migration::improve_concurrency(conn)?;
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
         CREATE TABLE IF NOT EXISTS discoveries (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            created_at_ms INTEGER NOT NULL,\
            agent_id TEXT NOT NULL,\
            title TEXT NOT NULL,\
            signal TEXT,\
            suggested_cmd TEXT\
         );\
         CREATE TABLE IF NOT EXISTS discovery_evidence (\
            discovery_id INTEGER NOT NULL,\
            ord INTEGER NOT NULL,\
            detail TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_discovery_evidence_discovery \
            ON discovery_evidence(discovery_id, ord);\
         CREATE TABLE IF NOT EXISTS surface_declarations (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            created_at_ms INTEGER NOT NULL,\
            agent_id TEXT NOT NULL,\
            kind TEXT NOT NULL,\
            scope TEXT NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS surface_tags (\
            declaration_id INTEGER NOT NULL,\
            ord INTEGER NOT NULL,\
            tag TEXT NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS surface_tokens (\
            declaration_id INTEGER NOT NULL,\
            ord INTEGER NOT NULL,\
            token TEXT NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS surface_paths (\
            declaration_id INTEGER NOT NULL,\
            ord INTEGER NOT NULL,\
            path TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_surface_declarations_kind_agent \
            ON surface_declarations(kind, agent_id, created_at_ms);\
         CREATE INDEX IF NOT EXISTS idx_surface_paths_path \
            ON surface_paths(path);\
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
    migrate_typed_message_storage(conn)?;
    prune(conn)?;
    Ok(())
}

/// Ensure agent state has the split seen/progress columns.
fn migrate_agent_state_columns(conn: &Connection) -> anyhow::Result<()> {
    let columns = table_columns(conn, "agent_state")?;
    if columns.contains("updated_at_ms") {
        let last_seen_expr = if columns.contains("last_seen_at_ms") {
            "CASE
                 WHEN last_seen_at_ms = 0 THEN updated_at_ms
                 ELSE last_seen_at_ms
             END"
        } else {
            "updated_at_ms"
        };
        let last_progress_expr = if columns.contains("last_progress_at_ms") {
            "CASE
                 WHEN last_progress_at_ms = 0 THEN updated_at_ms
                 ELSE last_progress_at_ms
             END"
        } else {
            "updated_at_ms"
        };
        conn.execute_batch(
            "ALTER TABLE agent_state RENAME TO agent_state_old;\
             CREATE TABLE agent_state (\
                agent_id TEXT PRIMARY KEY,\
                status TEXT,\
                plan TEXT,\
                last_seen_at_ms INTEGER NOT NULL DEFAULT 0,\
                last_progress_at_ms INTEGER NOT NULL DEFAULT 0\
             );",
        )?;
        conn.execute(
            &format!(
                "INSERT INTO agent_state(agent_id, status, plan, last_seen_at_ms, last_progress_at_ms)
                 SELECT agent_id, status, plan, {last_seen_expr}, {last_progress_expr}
                 FROM agent_state_old"
            ),
            [],
        )?;
        conn.execute("DROP TABLE agent_state_old", [])?;
        return Ok(());
    }
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
    Ok(())
}

/// Remove transcript-backed typed message rows after dedicated typed tables exist.
fn migrate_typed_message_storage(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM messages WHERE kind IN ('discovery', 'intent', 'declaration')",
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
