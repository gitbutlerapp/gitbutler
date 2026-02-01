use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20250717150441,
    "CREATE TABLE `workspace_rules`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`enabled` BOOL NOT NULL,
	`trigger` TEXT NOT NULL,
	`filters` TEXT NOT NULL,
	`action` TEXT NOT NULL
);",
)];

/// Tests are in `but-db/tests/db/table/workspace_rules.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRule {
    pub id: String,
    pub created_at: chrono::NaiveDateTime,
    pub enabled: bool,
    pub trigger: String,
    pub filters: String,
    pub action: String,
}

impl DbHandle {
    pub fn workspace_rules(&self) -> WorkspaceRulesHandle<'_> {
        WorkspaceRulesHandle { conn: &self.conn }
    }

    pub fn workspace_rules_mut(&mut self) -> WorkspaceRulesHandleMut<'_> {
        WorkspaceRulesHandleMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn workspace_rules(&self) -> WorkspaceRulesHandle<'_> {
        WorkspaceRulesHandle { conn: self.inner() }
    }

    pub fn workspace_rules_mut(&mut self) -> WorkspaceRulesHandleMut<'_> {
        WorkspaceRulesHandleMut { conn: self.inner() }
    }
}

pub struct WorkspaceRulesHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct WorkspaceRulesHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl WorkspaceRulesHandle<'_> {
    /// Get a WorkspaceRule by id (primary key)
    pub fn get(&self, id: &str) -> rusqlite::Result<Option<WorkspaceRule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, enabled, trigger, filters, action \
             FROM workspace_rules WHERE id = ?1",
        )?;

        let result = stmt
            .query_row([id], |row| {
                Ok(WorkspaceRule {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    enabled: row.get(2)?,
                    trigger: row.get(3)?,
                    filters: row.get(4)?,
                    action: row.get(5)?,
                })
            })
            .optional()?;

        Ok(result)
    }

    /// List all workspace rules
    pub fn list(&self) -> rusqlite::Result<Vec<WorkspaceRule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, enabled, trigger, filters, action FROM workspace_rules",
        )?;

        let results = stmt.query_map([], |row| {
            Ok(WorkspaceRule {
                id: row.get(0)?,
                created_at: row.get(1)?,
                enabled: row.get(2)?,
                trigger: row.get(3)?,
                filters: row.get(4)?,
                action: row.get(5)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }
}

impl WorkspaceRulesHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> WorkspaceRulesHandle<'_> {
        WorkspaceRulesHandle { conn: self.conn }
    }

    /// Insert a new WorkspaceRule
    pub fn insert(&mut self, rule: WorkspaceRule) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO workspace_rules (id, created_at, enabled, trigger, filters, action) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                rule.id,
                rule.created_at,
                rule.enabled,
                rule.trigger,
                rule.filters,
                rule.action,
            ],
        )?;
        Ok(())
    }

    /// Update an existing WorkspaceRule
    pub fn update(&mut self, id: &str, rule: WorkspaceRule) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE workspace_rules SET enabled = ?1, trigger = ?2, filters = ?3, action = ?4 \
             WHERE id = ?5",
            rusqlite::params![rule.enabled, rule.trigger, rule.filters, rule.action, id,],
        )?;
        Ok(())
    }

    /// Delete a WorkspaceRule by id
    pub fn delete(&mut self, id: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM workspace_rules WHERE id = ?1", [id])?;
        Ok(())
    }
}
