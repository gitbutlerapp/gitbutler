use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[
    M::up(
        20250703203544,
        "CREATE TABLE `claude_code_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`stack_id` TEXT NOT NULL
);",
    ),
    M::up(
        20250811130145,
        "CREATE TABLE `claude_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`current_id` TEXT NOT NULL UNIQUE,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL
);

CREATE INDEX index_claude_sessions_on_current_id ON claude_sessions (current_id);

CREATE TABLE `claude_messages`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`session_id` TEXT NOT NULL REFERENCES claude_sessions(id),
	`created_at` TIMESTAMP NOT NULL,
	`content_type` TEXT NOT NULL,
	`content` TEXT NOT NULL
);

CREATE INDEX index_claude_messages_on_session_id ON claude_messages (session_id);
CREATE INDEX index_claude_messages_on_created_at ON claude_messages (created_at);",
    ),
    M::up(20250812093543, "DROP TABLE IF EXISTS `claude_code_sessions`;"),
    M::up(
        20250817195624,
        "CREATE TABLE `claude_permission_requests`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL,
	`tool_name` TEXT NOT NULL,
	`input` TEXT NOT NULL,
	`approved` BOOL
);",
    ),
    M::up(
        20250821095340,
        "ALTER TABLE claude_sessions ADD COLUMN in_gui BOOLEAN NOT NULL DEFAULT FALSE;",
    ),
    M::up(
        20250821142109,
        "-- Add session_ids column to claude_sessions table to track all session IDs used
ALTER TABLE claude_sessions ADD COLUMN session_ids TEXT NOT NULL DEFAULT '[]';

-- Initialize existing sessions with their current_id in the session_ids array
UPDATE claude_sessions SET session_ids = json_array(current_id);",
    ),
    M::up(
        20251106102333,
        r#"-- Replace approved (bool) and scope (text) with a single decision (text) column
ALTER TABLE `claude_permission_requests` ADD COLUMN `decision` TEXT;

-- Migrate existing data: approved=true -> allowSession, approved=false -> denySession
UPDATE `claude_permission_requests`
SET `decision` = CASE
    WHEN `approved` = 1 THEN '"allowSession"'
    WHEN `approved` = 0 THEN '"denySession"'
    ELSE NULL
END;

-- Drop the old columns
ALTER TABLE `claude_permission_requests` DROP COLUMN `approved`;"#,
    ),
    M::up(
        20251107134016,
        "-- Add permissions columns to claude_sessions table to track approved and denied permissions
ALTER TABLE claude_sessions ADD COLUMN approved_permissions TEXT NOT NULL DEFAULT '[]';
ALTER TABLE claude_sessions ADD COLUMN denied_permissions TEXT NOT NULL DEFAULT '[]';",
    ),
    M::up(
        20251110103940,
        "-- Add use_wildcard column to claude_permission_requests table
ALTER TABLE `claude_permission_requests` ADD COLUMN `use_wildcard` BOOLEAN NOT NULL DEFAULT 0;",
    ),
];

/// Tests are in `but-db/tests/db/table/claude.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaudeSession {
    pub id: String,
    pub current_id: String,
    pub session_ids: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub in_gui: bool,
    pub approved_permissions: String,
    pub denied_permissions: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub id: String,
    pub session_id: String,
    pub created_at: chrono::NaiveDateTime,
    pub content_type: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaudePermissionRequest {
    pub id: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub tool_name: String,
    pub input: String,
    pub decision: Option<String>,
    pub use_wildcard: bool,
}

impl DbHandle {
    pub fn claude(&self) -> Claude<'_> {
        Claude { conn: &self.conn }
    }

    pub fn claude_mut(&mut self) -> ClaudeMut<'_> {
        ClaudeMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn claude(&self) -> Claude<'_> {
        Claude { conn: self.inner() }
    }

    pub fn claude_mut(&mut self) -> ClaudeMut<'_> {
        ClaudeMut { conn: self.inner() }
    }
}

pub struct Claude<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct ClaudeMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl Claude<'_> {
    // Permission Requests
    pub fn get_permission_request(&self, id: &str) -> rusqlite::Result<Option<ClaudePermissionRequest>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, updated_at, tool_name, input, decision, use_wildcard \
             FROM claude_permission_requests WHERE id = ?1",
        )?;

        let result = stmt
            .query_row([id], |row| {
                Ok(ClaudePermissionRequest {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    updated_at: row.get(2)?,
                    tool_name: row.get(3)?,
                    input: row.get(4)?,
                    decision: row.get(5)?,
                    use_wildcard: row.get(6)?,
                })
            })
            .optional()?;

        Ok(result)
    }

    pub fn list_permission_requests(&self) -> rusqlite::Result<Vec<ClaudePermissionRequest>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, updated_at, tool_name, input, decision, use_wildcard \
             FROM claude_permission_requests",
        )?;

        let results = stmt.query_map([], |row| {
            Ok(ClaudePermissionRequest {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                tool_name: row.get(3)?,
                input: row.get(4)?,
                decision: row.get(5)?,
                use_wildcard: row.get(6)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }

    // Sessions
    pub fn get_session(&self, id: &str) -> rusqlite::Result<Option<ClaudeSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, current_id, session_ids, created_at, updated_at, in_gui, approved_permissions, denied_permissions \
             FROM claude_sessions WHERE id = ?1",
        )?;

        let result = stmt
            .query_row([id], |row| {
                Ok(ClaudeSession {
                    id: row.get(0)?,
                    current_id: row.get(1)?,
                    session_ids: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    in_gui: row.get(5)?,
                    approved_permissions: row.get(6)?,
                    denied_permissions: row.get(7)?,
                })
            })
            .optional()?;

        Ok(result)
    }

    pub fn get_session_by_current_id(&self, current_id: &str) -> rusqlite::Result<Option<ClaudeSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, current_id, session_ids, created_at, updated_at, in_gui, approved_permissions, denied_permissions \
             FROM claude_sessions WHERE current_id = ?1",
        )?;

        let result = stmt
            .query_row([current_id], |row| {
                Ok(ClaudeSession {
                    id: row.get(0)?,
                    current_id: row.get(1)?,
                    session_ids: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    in_gui: row.get(5)?,
                    approved_permissions: row.get(6)?,
                    denied_permissions: row.get(7)?,
                })
            })
            .optional()?;

        Ok(result)
    }

    // Messages
    pub fn list_messages_by_session(&self, session_id: &str) -> rusqlite::Result<Vec<ClaudeMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, created_at, content_type, content \
             FROM claude_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )?;

        let results = stmt.query_map([session_id], |row| {
            Ok(ClaudeMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                created_at: row.get(2)?,
                content_type: row.get(3)?,
                content: row.get(4)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }

    pub fn get_message_of_type(
        &self,
        content_type: String,
        offset: Option<i64>,
    ) -> rusqlite::Result<Option<ClaudeMessage>> {
        let offset = offset.unwrap_or(0);
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, created_at, content_type, content \
             FROM claude_messages WHERE content_type = ?1 ORDER BY created_at DESC LIMIT 1 OFFSET ?2",
        )?;

        let result = stmt
            .query_row(rusqlite::params![content_type, offset], |row| {
                Ok(ClaudeMessage {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    created_at: row.get(2)?,
                    content_type: row.get(3)?,
                    content: row.get(4)?,
                })
            })
            .optional()?;

        Ok(result)
    }
}

impl ClaudeMut<'_> {
    // Permission Requests
    pub fn insert_permission_request(&mut self, request: ClaudePermissionRequest) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO claude_permission_requests (id, created_at, updated_at, tool_name, input, decision, use_wildcard) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                request.id,
                request.created_at,
                request.updated_at,
                request.tool_name,
                request.input,
                request.decision,
                request.use_wildcard,
            ],
        )?;
        Ok(())
    }

    pub fn set_permission_request_decision(&mut self, id: &str, decision: Option<String>) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE claude_permission_requests SET decision = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![decision, chrono::Local::now().naive_local(), id],
        )?;
        Ok(())
    }

    pub fn set_permission_request_decision_and_wildcard(
        &mut self,
        id: &str,
        decision: Option<String>,
        use_wildcard: bool,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE claude_permission_requests SET decision = ?1, use_wildcard = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![decision, use_wildcard, chrono::Local::now().naive_local(), id],
        )?;
        Ok(())
    }

    pub fn delete_permission_request(&mut self, id: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM claude_permission_requests WHERE id = ?1", [id])?;
        Ok(())
    }

    // Sessions
    pub fn insert_session(&mut self, session: ClaudeSession) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO claude_sessions (id, current_id, session_ids, created_at, updated_at, in_gui, approved_permissions, denied_permissions) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                session.id,
                session.current_id,
                session.session_ids,
                session.created_at,
                session.updated_at,
                session.in_gui,
                session.approved_permissions,
                session.denied_permissions,
            ],
        )?;
        Ok(())
    }

    pub fn update_session_current_id(&mut self, id: &str, current_id: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE claude_sessions SET current_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![current_id, chrono::Local::now().naive_local(), id],
        )?;
        Ok(())
    }

    pub fn update_session_ids(&mut self, id: &str, session_ids: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE claude_sessions SET session_ids = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![session_ids, chrono::Local::now().naive_local(), id],
        )?;
        Ok(())
    }

    pub fn update_session_in_gui(&mut self, id: &str, in_gui: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE claude_sessions SET in_gui = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![in_gui, chrono::Local::now().naive_local(), id],
        )?;
        Ok(())
    }

    pub fn update_session_permissions(
        &mut self,
        id: &str,
        approved_permissions: &str,
        denied_permissions: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE claude_sessions SET approved_permissions = ?1, denied_permissions = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![approved_permissions, denied_permissions, chrono::Local::now().naive_local(), id],
        )?;
        Ok(())
    }

    pub fn delete_session(&mut self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM claude_sessions WHERE id = ?1", [id])?;
        Ok(())
    }

    // Messages
    pub fn insert_message(&mut self, message: ClaudeMessage) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO claude_messages (id, session_id, created_at, content_type, content) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                message.id,
                message.session_id,
                message.created_at,
                message.content_type,
                message.content,
            ],
        )?;
        Ok(())
    }

    pub fn delete_messages_by_session(&mut self, session_id: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM claude_messages WHERE session_id = ?1", [session_id])?;
        Ok(())
    }

    pub fn delete_session_and_messages(&mut self, session_id: &str) -> rusqlite::Result<()> {
        // Here we'd have to use a savepoint, but the abstraction is a bit cumbersome
        // to use for *all other cases*, so we do a hand-rolled implementation here.
        self.conn.execute("SAVEPOINT delete_session_and_messages", [])?;
        let result = (|| {
            self.conn
                .execute("DELETE FROM claude_messages WHERE session_id = ?1", [session_id])?;
            self.conn
                .execute("DELETE FROM claude_sessions WHERE id = ?1", [session_id])?;
            Ok(())
        })();
        match result {
            Ok(_) => {
                self.conn.execute("RELEASE delete_session_and_messages", [])?;
                Ok(())
            }
            Err(e) => {
                self.conn.execute("ROLLBACK TO delete_session_and_messages", [])?;
                Err(e)
            }
        }
    }
}
