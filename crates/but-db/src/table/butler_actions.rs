use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[
    M::up(
        20250529110746,
        "CREATE TABLE `butler_actions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`external_prompt` TEXT NOT NULL,
	`handler` TEXT NOT NULL,
	`handler_prompt` TEXT,
	`snapshot_before` TEXT NOT NULL,
	`snapshot_after` TEXT NOT NULL,
	`response` TEXT,
	`error` TEXT
);

CREATE INDEX `idx_butler_actions_created_at` ON `butler_actions`(`created_at`);
",
    ),
    M::up(
        20250530112246,
        "ALTER TABLE `butler_actions` DROP COLUMN `external_prompt`;
ALTER TABLE `butler_actions` ADD COLUMN `external_summary` TEXT NOT NULL;
ALTER TABLE `butler_actions` ADD COLUMN `external_prompt` TEXT;
",
    ),
    M::up(
        20250616090656,
        "ALTER TABLE `butler_actions` ADD COLUMN `source` TEXT;",
    ),
    M::up(
        20250619181700,
        "ALTER TABLE `butler_actions` DROP COLUMN `handler_prompt`;",
    ),
];

/// Tests are in `but-db/tests/db/table/butler_actions.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButlerAction {
    /// UUID identifier of the action.
    pub id: String,
    /// The time when the action was performed.
    pub created_at: chrono::NaiveDateTime,
    /// The prompt that was used to generate the changes that were made, if applicable
    pub external_prompt: Option<String>,
    /// A description of the change that was made and why it was made - i.e. the information that can be obtained from the caller.
    pub external_summary: String,
    /// The handler / implementation that performed the action.
    pub handler: String,
    /// A GitBulter Oplog snapshot ID (git oid) before the action was performed.
    pub snapshot_before: String,
    /// A GitBulter Oplog snapshot ID (git oid) after the action was performed.
    pub snapshot_after: String,
    /// The outcome of the action, if it was successful.
    pub response: Option<String>,
    /// An error message if the action failed.
    pub error: Option<String>,
    /// The source of the action (e.g. "ButCli", "GitButler", "Mcp", "Unknown")
    pub source: Option<String>,
}

impl DbHandle {
    pub fn butler_actions(&self) -> ButlerActionsHandle<'_> {
        ButlerActionsHandle { conn: &self.conn }
    }

    pub fn butler_actions_mut(&mut self) -> ButlerActionsHandleMut<'_> {
        ButlerActionsHandleMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn butler_actions(&self) -> ButlerActionsHandle<'_> {
        ButlerActionsHandle { conn: self.inner() }
    }

    pub fn butler_actions_mut(&mut self) -> ButlerActionsHandleMut<'_> {
        ButlerActionsHandleMut { conn: self.inner() }
    }
}

pub struct ButlerActionsHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct ButlerActionsHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl ButlerActionsHandle<'_> {
    /// Lists butler actions with pagination, ordered by created_at descending.
    /// Returns a tuple of (total_count, actions).
    pub fn list(&self, offset: i64, limit: i64) -> rusqlite::Result<(i64, Vec<ButlerAction>)> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, external_prompt, external_summary, handler, \
             snapshot_before, snapshot_after, response, error, source \
             FROM butler_actions ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let actions = stmt
            .query_map([limit, offset], |row| {
                Ok(ButlerAction {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    external_prompt: row.get(2)?,
                    external_summary: row.get(3)?,
                    handler: row.get(4)?,
                    snapshot_before: row.get(5)?,
                    snapshot_after: row.get(6)?,
                    response: row.get(7)?,
                    error: row.get(8)?,
                    source: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM butler_actions", [], |row| row.get(0))?;

        Ok((total, actions))
    }
}

impl ButlerActionsHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> ButlerActionsHandle<'_> {
        ButlerActionsHandle { conn: self.conn }
    }

    /// Insert a new butler action.
    pub fn insert(&mut self, action: ButlerAction) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO butler_actions (id, created_at, external_prompt, external_summary, handler, \
             snapshot_before, snapshot_after, response, error, source) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                action.id,
                action.created_at,
                action.external_prompt,
                action.external_summary,
                action.handler,
                action.snapshot_before,
                action.snapshot_after,
                action.response,
                action.error,
                action.source,
            ],
        )?;
        Ok(())
    }
}
