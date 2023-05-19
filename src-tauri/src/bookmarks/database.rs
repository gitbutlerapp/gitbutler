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

    pub fn get_by_id(&self, id: &str) -> Result<Option<Bookmark>> {
        self.database.transaction(|tx| {
            let mut stmt = get_by_id_stmt(tx).context("Failed to prepare get_by_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! { ":id": id })
                .context("Failed to execute get_by_id statement")?;
            if let Some(row) = rows.next()? {
                Ok(Some(parse_row(row)?))
            } else {
                Ok(None)
            }
        })
    }

    pub fn upsert(&self, bookmark: &Bookmark) -> Result<()> {
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            let created_timestamp_ms = bookmark.created_timestamp_ms.to_string();
            let updated_timestamp_ms = bookmark.updated_timestamp_ms.to_string();
            stmt.execute(rusqlite::named_params! {
                ":id": &bookmark.id,
                ":project_id": &bookmark.project_id,
                ":created_timestamp_ms": &created_timestamp_ms,
                ":updated_timestamp_ms": &updated_timestamp_ms,
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

fn insert_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        INSERT INTO `bookmarks` (`id`, `project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `note`, `deleted`)
        VALUES (:id, :project_id, :created_timestamp_ms, :updated_timestamp_ms, :note, :deleted)
        ON CONFLICT(`id`) DO UPDATE SET
            `updated_timestamp_ms` = :updated_timestamp_ms,
            `note` = :note,
            `deleted` = :deleted
        ",
    )?)
}

fn get_by_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `id`, `project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `note`, `deleted`
        FROM `bookmarks`
        WHERE `id` = :id
        ",
    )?)
}

fn list_by_project_id_range_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `id`, `project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `note`, `deleted`
        FROM `bookmarks`
        WHERE `project_id` = :project_id
        AND `updated_timestamp_ms` >= :start
        AND `updated_timestamp_ms` < :end
        ORDER BY `created_timestamp_ms` DESC
        ",
    )?)
}

fn list_by_project_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `id`, `project_id`, `created_timestamp_ms`, `updated_timestamp_ms`, `note`, `deleted`
        FROM `bookmarks`
        WHERE `project_id` = :project_id
        ORDER BY `created_timestamp_ms` DESC
        ",
    )?)
}

fn parse_row(row: &rusqlite::Row) -> Result<Bookmark> {
    Ok(Bookmark {
        id: row.get(0).context("Failed to get id")?,
        project_id: row.get(1).context("Failed to get project_id")?,
        created_timestamp_ms: row
            .get::<usize, String>(2)
            .context("Failed to get created_timestamp_ms")?
            .parse::<u128>()
            .context("Failed to parse created_timestamp_ms")?,
        updated_timestamp_ms: row
            .get::<usize, String>(3)
            .context("Failed to get updated_timestamp_ms")?
            .parse::<u128>()
            .context("Failed to parse updated_timestamp_ms")?,
        note: row.get(4).context("Failed to get note")?,
        deleted: row.get(5).context("Failed to get deleted")?,
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
            id: "id".to_string(),
            project_id: "project_id".to_string(),
            created_timestamp_ms: 123,
            updated_timestamp_ms: 123,
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
            id: "id".to_string(),
            project_id: "project_id".to_string(),
            created_timestamp_ms: 123,
            updated_timestamp_ms: 123,
            note: "note".to_string(),
            deleted: false,
        };
        database.upsert(&bookmark_one)?;

        let bookmark_two = Bookmark {
            id: "id2".to_string(),
            project_id: "project_id".to_string(),
            created_timestamp_ms: 456,
            updated_timestamp_ms: 456,
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

        assert_eq!(database.get_by_id("id")?, None);

        let bookmark = Bookmark {
            id: "id".to_string(),
            project_id: "project_id".to_string(),
            created_timestamp_ms: 123,
            updated_timestamp_ms: 123,
            note: "note".to_string(),
            deleted: false,
        };

        database.upsert(&bookmark)?;
        assert_eq!(database.get_by_id(&bookmark.id)?, Some(bookmark.clone()));

        let updated = Bookmark {
            note: "updated".to_string(),
            updated_timestamp_ms: 456,
            ..bookmark.clone()
        };
        database.upsert(&updated)?;
        assert_eq!(database.get_by_id(&bookmark.id.clone())?, Some(updated));

        Ok(())
    }
}
