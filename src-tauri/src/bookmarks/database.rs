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

    pub fn insert(&self, bookmark: &Bookmark) -> Result<()> {
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            let timestamp_ms = bookmark.timestamp_ms.to_string();
            stmt.execute(rusqlite::named_params! {
                ":id": &bookmark.id,
                ":project_id": &bookmark.project_id,
                ":timestamp_ms": &timestamp_ms,
                ":note": &bookmark.note,
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
        INSERT INTO `bookmarks` (`id`, `project_id`, `timestamp_ms`, `note`)
        VALUES (:id, :project_id, :timestamp_ms, :note)
        ",
    )?)
}

fn list_by_project_id_range_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `id`, `project_id`, `timestamp_ms`, `note`
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
        SELECT `id`, `project_id`, `timestamp_ms`, `note`
        FROM `bookmarks`
        WHERE `project_id` = :project_id
        ORDER BY `timestamp_ms` DESC
        ",
    )?)
}

fn parse_row(row: &rusqlite::Row) -> Result<Bookmark> {
    Ok(Bookmark {
        id: row.get(0).context("Failed to get id")?,
        project_id: row.get(1).context("Failed to get project_id")?,
        timestamp_ms: row
            .get::<usize, String>(2)
            .context("Failed to get timestamp_ms")?
            .parse()
            .context("Failed to parse timestamp_ms")?,
        note: row.get(3).context("Failed to get note")?,
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
            timestamp_ms: 123,
            note: "note".to_string(),
        };

        database.insert(&bookmark)?;

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
            timestamp_ms: 123,
            note: "note".to_string(),
        };
        database.insert(&bookmark_one)?;

        let bookmark_two = Bookmark {
            id: "id2".to_string(),
            project_id: "project_id".to_string(),
            timestamp_ms: 456,
            note: "note".to_string(),
        };
        database.insert(&bookmark_two)?;

        let result = database.list_by_project_id_range(
            &bookmark_one.project_id,
            ops::Range { start: 0, end: 250 },
        )?;
        assert_eq!(result, vec![bookmark_one]);

        Ok(())
    }
}
