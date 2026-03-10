//! Service handlers for agent state mutations.

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::db;

/// Response payload for `done`.
#[derive(Debug, Serialize)]
pub(crate) struct DoneResponse {
    /// Standard success marker.
    pub ok: bool,
    /// Number of released claims.
    pub released_claims: i64,
    /// Cleared agent-state fields.
    pub cleared: Vec<String>,
}

/// Update the agent status.
pub(crate) fn set_status(
    conn: &Connection,
    agent_id: &str,
    value: Option<&str>,
) -> anyhow::Result<()> {
    update_agent_state_field(conn, agent_id, AgentStateField::Status, value)
}

/// Update the agent plan.
pub(crate) fn set_plan(
    conn: &Connection,
    agent_id: &str,
    value: Option<&str>,
) -> anyhow::Result<()> {
    update_agent_state_field(conn, agent_id, AgentStateField::Plan, value)
}

/// Finish work, clear status/plan, and drop the caller's claims.
pub(crate) fn done(
    conn: &Connection,
    agent_id: &str,
    summary: &str,
) -> anyhow::Result<DoneResponse> {
    let now_ms = db::now_unix_ms()?;
    let released: i64 = conn.query_row(
        "SELECT COUNT(1) FROM claims WHERE agent_id = ?1",
        params![agent_id],
        |row| row.get(0),
    )?;
    conn.execute("DELETE FROM claims WHERE agent_id = ?1", params![agent_id])?;
    db::ensure_agent_row(conn, agent_id)?;
    conn.execute(
        "UPDATE agent_state
         SET status = NULL,
             plan = NULL,
             last_seen_at_ms = ?2,
             last_progress_at_ms = ?2
         WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;
    let body_json = serde_json::json!({ "text": format!("DONE: {summary}") }).to_string();
    db::insert_history_message(conn, now_ms, agent_id, "message", &body_json)?;
    Ok(DoneResponse {
        ok: true,
        released_claims: released,
        cleared: vec!["status".to_owned(), "plan".to_owned()],
    })
}

/// Agent-state field selector.
#[derive(Clone, Copy)]
enum AgentStateField {
    /// Agent status field.
    Status,
    /// Agent plan field.
    Plan,
}

/// Update one agent-state field.
fn update_agent_state_field(
    conn: &Connection,
    agent_id: &str,
    field: AgentStateField,
    value: Option<&str>,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    db::ensure_agent_row(conn, agent_id)?;
    let sql = match field {
        AgentStateField::Status => {
            "UPDATE agent_state
             SET status = ?2, last_seen_at_ms = ?3, last_progress_at_ms = ?3
             WHERE agent_id = ?1"
        }
        AgentStateField::Plan => {
            "UPDATE agent_state
             SET plan = ?2, last_seen_at_ms = ?3, last_progress_at_ms = ?3
             WHERE agent_id = ?1"
        }
    };
    conn.execute(sql, params![agent_id, value, now_ms])?;
    Ok(())
}
