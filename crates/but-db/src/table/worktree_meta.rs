#![allow(missing_docs)]

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[
    M::up(
        20260715161258,
        SchemaVersion::Zero,
        "CREATE TABLE `worktree_meta`(
	`name` BLOB NOT NULL PRIMARY KEY,
	`archived` BOOL NOT NULL DEFAULT FALSE
);",
    ),
    M::up(
        20260716175500,
        SchemaVersion::Zero,
        "CREATE TABLE `worktree_adoption`(
	`id` INTEGER PRIMARY KEY CHECK (`id` = 1),
	`adopted_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);",
    ),
];

/// Tests are in `but-db/tests/db/table/worktree_meta.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorktreeMeta {
    /// The git worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub name: Vec<u8>,
    /// Whether the worktree is hidden from listings and graph traversal.
    pub archived: bool,
}

impl DbHandle {
    pub fn worktree_meta(&self) -> WorktreeMetaHandle<'_> {
        WorktreeMetaHandle { conn: &self.conn }
    }

    pub fn worktree_meta_mut(&mut self) -> WorktreeMetaHandleMut<'_> {
        WorktreeMetaHandleMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn worktree_meta(&self) -> WorktreeMetaHandle<'_> {
        WorktreeMetaHandle { conn: self.inner() }
    }

    pub fn worktree_meta_mut(&mut self) -> WorktreeMetaHandleMut<'_> {
        WorktreeMetaHandleMut { conn: self.inner() }
    }
}

pub struct WorktreeMetaHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct WorktreeMetaHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl WorktreeMetaHandle<'_> {
    /// Get a WorktreeMeta entry by name (primary key).
    pub fn get(&self, name: &[u8]) -> rusqlite::Result<Option<WorktreeMeta>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, archived FROM worktree_meta WHERE name = ?1")?;

        let result = stmt
            .query_row([name], |row| {
                Ok(WorktreeMeta {
                    name: row.get(0)?,
                    archived: row.get(1)?,
                })
            })
            .optional()?;

        Ok(result)
    }

    /// List all WorktreeMeta entries.
    pub fn list(&self) -> rusqlite::Result<Vec<WorktreeMeta>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, archived FROM worktree_meta ORDER BY name")?;

        let rows = stmt.query_map([], |row| {
            Ok(WorktreeMeta {
                name: row.get(0)?,
                archived: row.get(1)?,
            })
        })?;

        rows.collect()
    }

    /// Whether the one-time adoption of pre-existing worktrees has run for this
    /// project - see [`WorktreeMetaHandleMut::mark_adopted()`].
    pub fn adoption_ran(&self) -> rusqlite::Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM worktree_adoption WHERE id = 1)",
            [],
            |row| row.get(0),
        )
    }
}

impl WorktreeMetaHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> WorktreeMetaHandle<'_> {
        WorktreeMetaHandle { conn: self.conn }
    }

    /// Insert or replace a WorktreeMeta entry.
    pub fn upsert(&mut self, meta: WorktreeMeta) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO worktree_meta (name, archived) VALUES (?1, ?2)",
            rusqlite::params![meta.name, meta.archived],
        )?;
        Ok(())
    }

    /// Forget the linked worktree named `name`, so a worktree created under that
    /// name later starts out active. No-op for an unknown name.
    pub fn delete(&mut self, name: &[u8]) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM worktree_meta WHERE name = ?1", [name])?;
        Ok(())
    }

    /// Record that the one-time adoption of pre-existing worktrees has run.
    ///
    /// Idempotent - marking again has no effect. The row's `adopted_at` column is
    /// filled by the database as a debugging breadcrumb; nothing reads it.
    pub fn mark_adopted(&mut self) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO worktree_adoption (id) VALUES (1)",
            [],
        )?;
        Ok(())
    }
}
