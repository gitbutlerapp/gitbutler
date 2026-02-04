//! Sessions table operations.
//!
//! Maps Claude Code PIDs to agent IDs, enabling hooks to identify
//! which agent they belong to via process tree walking.

use chrono::{DateTime, Utc};

use crate::db::DbHandle;
use crate::db::migration::M;

/// Migrations for the sessions table.
pub const M: &[M<'static>] = &[M::up(
    20260206100000,
    "CREATE TABLE sessions (
        claude_pid INTEGER PRIMARY KEY,
        agent_id TEXT NOT NULL,
        registered_at TIMESTAMP NOT NULL
    );",
)];

impl DbHandle {
    /// Register or update a session mapping a Claude PID to an agent ID.
    pub fn register_session(&self, claude_pid: u32, agent_id: &str, now: DateTime<Utc>) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (claude_pid, agent_id, registered_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(claude_pid) DO UPDATE SET
                 agent_id = ?2,
                 registered_at = ?3",
            rusqlite::params![claude_pid as i64, agent_id, now],
        )?;
        Ok(())
    }

    /// Look up the agent ID for a given Claude PID.
    pub fn get_session_agent(&self, claude_pid: u32) -> rusqlite::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT agent_id FROM sessions WHERE claude_pid = ?1")?;
        let mut rows = stmt.query_map([claude_pid as i64], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(agent_id)) => Ok(Some(agent_id)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Delete sessions older than the given cutoff.
    pub fn expire_stale_sessions(&self, older_than: DateTime<Utc>) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM sessions WHERE registered_at < ?1",
            rusqlite::params![older_than],
        )
    }
}
