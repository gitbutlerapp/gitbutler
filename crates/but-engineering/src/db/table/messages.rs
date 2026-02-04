//! Messages table operations.

use chrono::{DateTime, Utc};

use crate::db::DbHandle;
use crate::db::migration::M;
use crate::types::{Message, MessageKind};

/// Migrations for the messages table.
pub const M: &[M<'static>] = &[
    M::up(
        20260204100001,
        "CREATE TABLE messages (
            id TEXT NOT NULL PRIMARY KEY,
            agent_id TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp TIMESTAMP NOT NULL
        );
        CREATE INDEX idx_messages_timestamp ON messages (timestamp);",
    ),
    M::up(
        20260205110001,
        "ALTER TABLE messages ADD COLUMN kind TEXT NOT NULL DEFAULT 'message';",
    ),
];

impl DbHandle {
    /// Insert a new message.
    pub fn insert_message(&self, message: &Message) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO messages (id, agent_id, content, timestamp, kind)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                message.id,
                message.agent_id,
                message.content,
                message.timestamp,
                message.kind.as_str()
            ],
        )?;
        Ok(())
    }

    /// Query messages since a given timestamp.
    pub fn query_messages_since(&self, since: DateTime<Utc>, limit: Option<usize>) -> rusqlite::Result<Vec<Message>> {
        let query = match limit {
            Some(_) => {
                "SELECT id, agent_id, content, timestamp, kind FROM messages
                 WHERE timestamp > ?1
                 ORDER BY timestamp ASC
                 LIMIT ?2"
            }
            None => {
                "SELECT id, agent_id, content, timestamp, kind FROM messages
                 WHERE timestamp > ?1
                 ORDER BY timestamp ASC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let rows = match limit {
            Some(l) => stmt.query_map(rusqlite::params![since, l as i64], row_to_message)?,
            None => stmt.query_map([since], row_to_message)?,
        };

        rows.collect()
    }

    /// Query recent messages within a time window, with limit.
    pub fn query_recent_messages(&self, since: DateTime<Utc>, limit: usize) -> rusqlite::Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, timestamp, kind FROM (
                SELECT id, agent_id, content, timestamp, kind FROM messages
                WHERE timestamp > ?1
                ORDER BY timestamp DESC
                LIMIT ?2
            ) ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(rusqlite::params![since, limit as i64], row_to_message)?;
        rows.collect()
    }

    /// Query recent block messages within a time window.
    pub fn query_recent_blocks(&self, since: DateTime<Utc>, limit: usize) -> rusqlite::Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, timestamp, kind FROM (
                SELECT id, agent_id, content, timestamp, kind FROM messages
                WHERE timestamp > ?1 AND kind = 'block'
                ORDER BY timestamp DESC
                LIMIT ?2
            ) ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(rusqlite::params![since, limit as i64], row_to_message)?;
        rows.collect()
    }

    /// Query recent discoveries within a time window.
    pub fn query_recent_discoveries(&self, since: DateTime<Utc>, limit: usize) -> rusqlite::Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, timestamp, kind FROM (
                SELECT id, agent_id, content, timestamp, kind FROM messages
                WHERE timestamp > ?1 AND kind = 'discovery'
                ORDER BY timestamp DESC
                LIMIT ?2
            ) ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(rusqlite::params![since, limit as i64], row_to_message)?;
        rows.collect()
    }
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let kind_str: Option<String> = row.get(4)?;
    Ok(Message {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        content: row.get(2)?,
        timestamp: row.get(3)?,
        kind: MessageKind::from_db_str(kind_str.as_deref().unwrap_or("message")),
    })
}
