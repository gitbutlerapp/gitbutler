//! Agents table operations.

use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::db::DbHandle;
use crate::db::migration::M;
use crate::types::Agent;

/// Migrations for the agents table.
pub const M: &[M<'static>] = &[
    M::up(
        20260204100000,
        "CREATE TABLE agents (
            id TEXT NOT NULL PRIMARY KEY,
            status TEXT,
            last_active TIMESTAMP NOT NULL,
            last_read TIMESTAMP
        );
        CREATE INDEX idx_agents_last_active ON agents (last_active);",
    ),
    M::up(
        20260205110000,
        "ALTER TABLE agents ADD COLUMN plan TEXT;
         ALTER TABLE agents ADD COLUMN plan_updated_at TIMESTAMP;",
    ),
];

impl DbHandle {
    /// Upsert an agent, updating last_active.
    pub fn upsert_agent(&self, id: &str, now: DateTime<Utc>) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO agents (id, last_active)
             VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                 last_active = ?2",
            rusqlite::params![id, now],
        )?;
        Ok(())
    }

    /// Get an agent by ID.
    pub fn get_agent(&self, id: &str) -> rusqlite::Result<Option<Agent>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, status, last_active, last_read, plan, plan_updated_at FROM agents WHERE id = ?1")?;

        stmt.query_row([id], row_to_agent).optional()
    }

    /// Set or clear an agent's plan.
    pub fn set_agent_plan(&self, id: &str, plan: Option<&str>, now: DateTime<Utc>) -> rusqlite::Result<()> {
        let plan_updated_at: Option<DateTime<Utc>> = plan.map(|_| now);
        self.conn.execute(
            "UPDATE agents SET plan = ?1, plan_updated_at = ?2, last_active = ?3 WHERE id = ?4",
            rusqlite::params![plan, plan_updated_at, now, id],
        )?;
        Ok(())
    }

    /// Set or clear agent status.
    pub fn set_agent_status(&self, id: &str, status: Option<&str>, now: DateTime<Utc>) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE agents SET status = ?1, last_active = ?2 WHERE id = ?3",
            rusqlite::params![status, now, id],
        )?;
        Ok(())
    }

    /// Clear agent status without updating last_active.
    /// Used by stale-status cleanup paths to avoid making agents appear active.
    pub fn clear_agent_status(&self, id: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE agents SET status = NULL WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    /// Clear plans from agents that haven't been active since the given time.
    pub fn clear_stale_plans(&self, older_than: DateTime<Utc>) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE agents SET plan = NULL, plan_updated_at = NULL
             WHERE last_active < ?1 AND plan IS NOT NULL",
            rusqlite::params![older_than],
        )
    }

    /// Update agent's last_read timestamp.
    pub fn update_agent_last_read(&self, id: &str, last_read: DateTime<Utc>) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE agents SET last_read = ?1 WHERE id = ?2",
            rusqlite::params![last_read, id],
        )?;
        Ok(())
    }

    /// List agents, optionally filtered by activity within a duration.
    pub fn list_agents(&self, active_since: Option<DateTime<Utc>>) -> rusqlite::Result<Vec<Agent>> {
        let query = match active_since {
            Some(_) => {
                "SELECT id, status, last_active, last_read, plan, plan_updated_at FROM agents
                 WHERE last_active >= ?1
                 ORDER BY last_active DESC"
            }
            None => {
                "SELECT id, status, last_active, last_read, plan, plan_updated_at FROM agents
                 ORDER BY last_active DESC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let rows = match active_since {
            Some(since) => stmt.query_map([since], row_to_agent)?,
            None => stmt.query_map([], row_to_agent)?,
        };

        rows.collect()
    }
}

fn row_to_agent(row: &rusqlite::Row<'_>) -> rusqlite::Result<Agent> {
    Ok(Agent {
        id: row.get(0)?,
        status: row.get(1)?,
        last_active: row.get(2)?,
        last_read: row.get(3)?,
        plan: row.get(4)?,
        plan_updated_at: row.get(5)?,
    })
}
