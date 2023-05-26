use std::ops;

use anyhow::{Context, Result};

use crate::database;

use super::Bookmark;

#[derive(Clone)]
pub struct Database {
    database: database::Database,
}

impl Database {
    pub fn new(database: database::Database) -> Self {
        Self { database }
    }

    fn get_by_project_id_timestamp_ms(
        &self,
        project_id: &str,
        timestamp_ms: &u128,
    ) -> Result<Option<Bookmark>> {
        self.database.transaction(|tx| {
            let mut stmt = get_by_project_id_timestamp_ms_stmt(tx)
                .context("Failed to prepare get_by_project_id_timestamp_ms statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":project_id": project_id,
                    ":timestamp_ms": timestamp_ms.to_string(),
                })
                .context("Failed to execute get_by_project_id_timestamp_ms statement")?;
            if let Some(row) = rows.next()? {
                let bookmark = parse_row(row)?;
                Ok(Some(bookmark))
            } else {
                Ok(None)
            }
        })
    }

    pub fn upsert(&self, bookmark: &Bookmark) -> Result<Option<Bookmark>> {
        let existing = self
            .get_by_project_id_timestamp_ms(&bookmark.project_id, &bookmark.timestamp_ms)
            .context("Failed to get bookmark")?;
        if let Some(existing) = existing {
            if existing.updated_timestamp_ms >= bookmark.updated_timestamp_ms {
                return Ok(None);
            }
            self.update(bookmark).context("Failed to update bookmark")?;
            Ok(Some(bookmark.clone()))
        } else {
            self.insert(bookmark).context("Failed to insert bookmark")?;
            Ok(Some(bookmark.clone()))
        }
    }

    fn update(&self, bookmark: &Bookmark) -> Result<()> {
        self.database.transaction(|tx| {
            let mut stmt = update_bookmark_by_project_id_timestamp_ms_stmt(tx)
                .context("Failed to prepare update statement")?;
            stmt.execute(rusqlite::named_params! {
                ":project_id": &bookmark.project_id,
                ":timestamp_ms": &bookmark.timestamp_ms.to_string(),
                ":updated_timestamp_ms": &bookmark.updated_timestamp_ms.to_string(),
                ":note": &bookmark.note,
                ":deleted": &bookmark.deleted,
            })
            .context("Failed to execute update statement")?;
            Ok(())
        })
    }

    fn insert(&self, bookmark: &Bookmark) -> Result<()> {
        self.database.transaction(|tx| {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            stmt.execute(rusqlite::named_params! {
                ":project_id": &bookmark.project_id,
                ":timestamp_ms": &bookmark.timestamp_ms.to_string(),
                ":created_timestamp_ms": &bookmark.created_timestamp_ms.to_string(),
                ":updated_timestamp_ms": &bookmark.updated_timestamp_ms.to_string(),
                ":note": &bookmark.note,
                ":deleted": &bookmark.deleted,
            })
            .context("Failed to execute insert statement")?;
            Ok(())
        })
    }

    fn list_by_project_id_range(
        &self,
        project_id: &str,
        range: ops::Range<u128>,
    ) -> Result<Vec<Bookmark>> {
        self.database.transaction(|tx| {
            let mut stmt = list_by_project_id_range_stmt(tx)
                .context("Failed to prepare list_by_project_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":project_id": project_id,
                    ":start": range.start.to_string(),
                    ":end": range.end.to_string(),
                })
                .context("Failed to execute list_by_project_id statement")?;
            let mut bookmarks = Vec::new();
            while let Some(row) = rows.next()? {
                bookmarks.push(parse_row(row)?);
            }
            Ok(bookmarks)
        })
    }

    fn list_by_project_id_all(&self, project_id: &str) -> Result<Vec<Bookmark>> {
        self.database.transaction(|tx| {
            let mut stmt = list_by_project_id_stmt(tx)
                .context("Failed to prepare list_by_project_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! { ":project_id": project_id })
                .context("Failed to execute list_by_project_id statement")?;
            let mut bookmarks = Vec::new();
            while let Some(row) = rows.next()? {
                bookmarks.push(parse_row(row)?);
            }
            Ok(bookmarks)
        })
    }

    pub fn list_by_project_id(
        &self,
        project_id: &str,
        range: Option<ops::Range<u128>>,
    ) -> Result<Vec<Bookmark>> {
        if range.is_some() {
            self.list_by_project_id_range(project_id, range.unwrap())
        } else {
            self.list_by_project_id_all(project_id)
        }
    }
}

fn get_by_project_id_timestamp_ms_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `note`, `deleted`, `timestamp_ms`
        FROM `bookmarks`
        WHERE `project_id` = :project_id
        AND `timestamp_ms` = :timestamp_ms
        ",
    )?)
}

fn update_bookmark_by_project_id_timestamp_ms_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        UPDATE `bookmarks`
        SET `updated_timestamp_ms` = :updated_timestamp_ms,
            `note` = :note,
            `deleted` = :deleted
        WHERE `project_id` = :project_id
        AND `timestamp_ms` = :timestamp_ms
        ",
    )?)
}

fn insert_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        INSERT INTO `bookmarks` (`project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `timestamp_ms`, `note`, `deleted`)
        VALUES (:project_id, :created_timestamp_ms, :updated_timestamp_ms, :timestamp_ms, :note, :deleted)
        ",
    )?)
}

fn list_by_project_id_range_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `note`, `deleted`, `timestamp_ms`
        FROM `bookmarks`
        WHERE `project_id` = :project_id
        AND `timestamp_ms` >= :start
        AND `timestamp_ms` < :end
        ORDER BY `timestamp_ms` DESC
        ",
    )?)
}

fn list_by_project_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `note`, `deleted`, `timestamp_ms`
        FROM `bookmarks`
        WHERE `project_id` = :project_id
        ORDER BY `created_timestamp_ms` DESC
        ",
    )?)
}

fn parse_row(row: &rusqlite::Row) -> Result<Bookmark> {
    Ok(Bookmark {
        project_id: row.get(0).context("Failed to get project_id")?,
        created_timestamp_ms: row
            .get::<usize, String>(1)
            .context("Failed to get created_timestamp_ms")?
            .parse::<u128>()
            .context("Failed to parse created_timestamp_ms")?,
        updated_timestamp_ms: row
            .get::<usize, String>(2)
            .context("Failed to get updated_timestamp_ms")?
            .parse::<u128>()
            .context("Failed to parse updated_timestamp_ms")?,
        note: row.get(3).context("Failed to get note")?,
        deleted: row.get(4).context("Failed to get deleted")?,
        timestamp_ms: row
            .get::<usize, String>(5)
            .context("Failed to get timestamp_ms")?
            .parse::<u128>()
            .context("Failed to parse timestamp_ms")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_by_project_id_all() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let bookmark = Bookmark {
            project_id: "project_id".to_string(),
            timestamp_ms: 123,
            created_timestamp_ms: 0,
            updated_timestamp_ms: 0,
            note: "note".to_string(),
            deleted: false,
        };

        database.upsert(&bookmark)?;

        let result = database.list_by_project_id_all(&bookmark.project_id)?;

        assert_eq!(result, vec![bookmark]);

        Ok(())
    }

    #[test]
    fn list_by_project_id_range() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let bookmark_one = Bookmark {
            project_id: "project_id".to_string(),
            timestamp_ms: 123,
            created_timestamp_ms: 0,
            updated_timestamp_ms: 0,
            note: "note".to_string(),
            deleted: false,
        };
        database.upsert(&bookmark_one)?;

        let bookmark_two = Bookmark {
            project_id: "project_id".to_string(),
            timestamp_ms: 456,
            created_timestamp_ms: 0,
            updated_timestamp_ms: 1,
            note: "note".to_string(),
            deleted: false,
        };
        database.upsert(&bookmark_two)?;

        let result = database.list_by_project_id_range(
            &bookmark_one.project_id,
            ops::Range { start: 0, end: 250 },
        )?;
        assert_eq!(result, vec![bookmark_one]);

        Ok(())
    }

    #[test]
    fn update() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let bookmark = Bookmark {
            project_id: "project_id".to_string(),
            timestamp_ms: 123,
            created_timestamp_ms: 0,
            updated_timestamp_ms: 0,
            note: "note".to_string(),
            deleted: false,
        };

        database.upsert(&bookmark)?;
        assert_eq!(
            database.list_by_project_id_all(&bookmark.project_id)?,
            vec![bookmark.clone()]
        );

        let updated = Bookmark {
            note: "updated".to_string(),
            deleted: true,
            ..bookmark.clone()
        };
        database.upsert(&updated)?;
        assert_eq!(
            database.list_by_project_id_all(&bookmark.project_id)?,
            vec![updated.clone()]
        );

        Ok(())
    }
}
