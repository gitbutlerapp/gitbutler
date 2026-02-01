use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20251015212443,
    "CREATE TABLE `gerrit_metadata`(
	`change_id` TEXT NOT NULL PRIMARY KEY,
	`commit_id` TEXT NOT NULL,
	`review_url` TEXT NOT NULL,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL
);",
)];

/// Tests are in `but-db/tests/db/table/gerrit_metadata.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GerritMeta {
    /// Unique Gerrit change ID (primary key)
    pub change_id: String,
    /// Git commit ID
    pub commit_id: String,
    /// Gerrit review URL
    pub review_url: String,
    /// The time when the metadata was created
    pub created_at: chrono::NaiveDateTime,
    /// The time when the metadata was last updated
    pub updated_at: chrono::NaiveDateTime,
}

impl DbHandle {
    pub fn gerrit_metadata(&self) -> GerritMetadataHandle<'_> {
        GerritMetadataHandle { conn: &self.conn }
    }

    pub fn gerrit_metadata_mut(&mut self) -> GerritMetadataHandleMut<'_> {
        GerritMetadataHandleMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn gerrit_metadata(&self) -> GerritMetadataHandle<'_> {
        GerritMetadataHandle { conn: self.inner() }
    }

    pub fn gerrit_metadata_mut(&mut self) -> GerritMetadataHandleMut<'_> {
        GerritMetadataHandleMut { conn: self.inner() }
    }
}

pub struct GerritMetadataHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct GerritMetadataHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl GerritMetadataHandle<'_> {
    /// Get a GerritMeta entry by change_id (primary key)
    pub fn get(&self, change_id: &str) -> rusqlite::Result<Option<GerritMeta>> {
        let mut stmt = self.conn.prepare(
            "SELECT change_id, commit_id, review_url, created_at, updated_at \
             FROM gerrit_metadata WHERE change_id = ?1",
        )?;

        let result = stmt
            .query_row([change_id], |row| {
                Ok(GerritMeta {
                    change_id: row.get(0)?,
                    commit_id: row.get(1)?,
                    review_url: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .optional()?;

        Ok(result)
    }
}

impl GerritMetadataHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> GerritMetadataHandle<'_> {
        GerritMetadataHandle { conn: self.conn }
    }

    /// Insert a new GerritMeta entry
    pub fn insert(&mut self, meta: GerritMeta) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO gerrit_metadata (change_id, commit_id, review_url, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                meta.change_id,
                meta.commit_id,
                meta.review_url,
                meta.created_at,
                meta.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Update an existing GerritMeta entry
    pub fn update(&mut self, meta: GerritMeta) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE gerrit_metadata SET commit_id = ?1, review_url = ?2, updated_at = ?3 \
             WHERE change_id = ?4",
            rusqlite::params![
                meta.commit_id,
                meta.review_url,
                meta.updated_at,
                meta.change_id,
            ],
        )?;
        Ok(())
    }
}
