//! Claims table operations.
//!
//! File claims let agents declare ownership of files they're editing.
//! The PreToolUse hook checks claims to block edits to files owned by
//! other active agents.

use chrono::{DateTime, Utc};

use crate::db::DbHandle;
use crate::db::migration::M;
use crate::types::Claim;

/// Migrations for the claims table.
pub const M: &[M<'static>] = &[M::up(
    20260205100000,
    "CREATE TABLE claims (
        file_path TEXT NOT NULL,
        agent_id TEXT NOT NULL,
        claimed_at TIMESTAMP NOT NULL,
        PRIMARY KEY (file_path, agent_id)
    );
    CREATE INDEX idx_claims_agent_id ON claims (agent_id);
    CREATE INDEX idx_claims_claimed_at ON claims (claimed_at);",
)];

impl DbHandle {
    /// Claim one or more files for an agent. Upserts: re-claiming refreshes the timestamp.
    pub fn claim_files(&self, paths: &[&str], agent_id: &str, now: DateTime<Utc>) -> rusqlite::Result<()> {
        for path in paths {
            self.conn.execute(
                "INSERT INTO claims (file_path, agent_id, claimed_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(file_path, agent_id) DO UPDATE SET
                     claimed_at = ?3",
                rusqlite::params![path, agent_id, now],
            )?;
        }
        Ok(())
    }

    /// Release specific files for an agent.
    pub fn release_files(&self, paths: &[&str], agent_id: &str) -> rusqlite::Result<usize> {
        let mut count = 0;
        for path in paths {
            count += self.conn.execute(
                "DELETE FROM claims WHERE file_path = ?1 AND agent_id = ?2",
                rusqlite::params![path, agent_id],
            )?;
        }
        Ok(count)
    }

    /// Release all claims for an agent.
    pub fn release_all_for_agent(&self, agent_id: &str) -> rusqlite::Result<usize> {
        self.conn
            .execute("DELETE FROM claims WHERE agent_id = ?1", rusqlite::params![agent_id])
    }

    /// Get all claims for a specific file path.
    pub fn get_claims_for_file(&self, file_path: &str) -> rusqlite::Result<Vec<Claim>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file_path, agent_id, claimed_at FROM claims WHERE file_path = ?1")?;
        let rows = stmt.query_map([file_path], row_to_claim)?;
        rows.collect()
    }

    /// List all claims, optionally filtering to agents active since a given time.
    pub fn list_claims(&self, active_since: Option<DateTime<Utc>>) -> rusqlite::Result<Vec<Claim>> {
        match active_since {
            Some(since) => {
                let mut stmt = self.conn.prepare(
                    "SELECT c.file_path, c.agent_id, c.claimed_at FROM claims c
                     INNER JOIN agents a ON c.agent_id = a.id
                     WHERE a.last_active >= ?1
                     ORDER BY c.agent_id, c.file_path",
                )?;
                let rows = stmt.query_map([since], row_to_claim)?;
                rows.collect()
            }
            None => {
                let mut stmt = self
                    .conn
                    .prepare("SELECT file_path, agent_id, claimed_at FROM claims ORDER BY agent_id, file_path")?;
                let rows = stmt.query_map([], row_to_claim)?;
                rows.collect()
            }
        }
    }

    /// Delete claims from agents that haven't been active since the given time.
    pub fn expire_stale_claims(&self, older_than: DateTime<Utc>) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM claims WHERE agent_id IN (
                SELECT id FROM agents WHERE last_active < ?1
            )",
            rusqlite::params![older_than],
        )
    }

    /// Delete claim leases older than the given timestamp.
    pub fn expire_claims_older_than(&self, older_than: DateTime<Utc>) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM claims WHERE claimed_at < ?1",
            rusqlite::params![older_than],
        )
    }
}

fn row_to_claim(row: &rusqlite::Row<'_>) -> rusqlite::Result<Claim> {
    Ok(Claim {
        file_path: row.get(0)?,
        agent_id: row.get(1)?,
        claimed_at: row.get(2)?,
    })
}
