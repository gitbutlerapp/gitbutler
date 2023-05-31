use anyhow::{Context, Result};

use crate::database;

use super::session;

#[derive(Clone)]
pub struct Database {
    database: database::Database,
}

impl Database {
    pub fn new(database: database::Database) -> Self {
        Self { database }
    }

    pub fn insert(&self, project_id: &str, sessions: &[&session::Session]) -> Result<()> {
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            for session in sessions {
                stmt.execute(rusqlite::named_params! {
                    ":id": session.id,
                    ":project_id": project_id,
                    ":hash": session.hash,
                    ":branch": session.meta.branch,
                    ":commit": session.meta.commit,
                    ":start_timestamp_ms": session.meta.start_timestamp_ms.to_string(),
                    ":last_timestamp_ms": session.meta.last_timestamp_ms.to_string(),
                })
                .context("Failed to execute insert statement")?;
            }
            Ok(())
        })?;

        Ok(())
    }

    pub fn list_by_project_id(
        &self,
        project_id: &str,
        earliest_timestamp_ms: Option<u128>,
    ) -> Result<Vec<session::Session>> {
        self.database.transaction(|tx| {
            let mut stmt = list_by_project_id_stmt(tx)
                .context("Failed to prepare list_by_project_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":project_id": project_id,
                })
                .context("Failed to execute list_by_project_id statement")?;

            let mut sessions = Vec::new();
            while let Some(row) = rows
                .next()
                .context("Failed to iterate over list_by_project_id results")?
            {
                let session = parse_row(row)?;

                if let Some(earliest_timestamp_ms) = earliest_timestamp_ms {
                    if session.meta.last_timestamp_ms < earliest_timestamp_ms {
                        continue;
                    }
                }

                sessions.push(session);
            }
            Ok(sessions)
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<session::Session>> {
        self.database.transaction(|tx| {
            let mut stmt = get_by_id_stmt(tx).context("Failed to prepare get_by_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":id": id,
                })
                .context("Failed to execute get_by_id statement")?;
            if let Some(row) = rows
                .next()
                .context("Failed to iterate over get_by_id results")?
            {
                Ok(Some(parse_row(row)?))
            } else {
                Ok(None)
            }
        })
    }
}

fn parse_row(row: &rusqlite::Row) -> Result<session::Session> {
    Ok(session::Session {
        id: row.get(0).context("Failed to get id")?,
        hash: row.get(2).context("Failed to get hash")?,
        meta: session::Meta {
            branch: row.get(3).context("Failed to get branch")?,
            commit: row.get(4).context("Failed to get commit")?,
            start_timestamp_ms: row
                .get::<usize, String>(5)
                .context("Failed to get start_timestamp_ms")?
                .parse()
                .context("Failed to parse start_timestamp_ms")?,
            last_timestamp_ms: row
                .get::<usize, String>(6)
                .context("Failed to get last_timestamp_ms")?
                .parse()
                .context("Failed to parse last_timestamp_ms")?,
        },
    })
}

fn list_by_project_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `id`, `project_id`, `hash`, `branch`, `commit`, `start_timestamp_ms`, `last_timestamp_ms` FROM `sessions` WHERE `project_id` = :project_id ORDER BY `start_timestamp_ms` DESC",
    )?)
}

fn get_by_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `id`, `project_id`, `hash`, `branch`, `commit`, `start_timestamp_ms`, `last_timestamp_ms` FROM `sessions` WHERE `id` = :id",
    )?)
}

fn insert_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "INSERT INTO 'sessions' (
            `id`, `project_id`, `hash`, `branch`, `commit`, `start_timestamp_ms`, `last_timestamp_ms`
        ) VALUES (
            :id, :project_id, :hash, :branch, :commit, :start_timestamp_ms, :last_timestamp_ms
        ) ON CONFLICT(`id`) DO UPDATE SET
            `project_id` = :project_id,
            `hash` = :hash,
            `branch` = :branch,
            `commit` = :commit,
            `start_timestamp_ms` = :start_timestamp_ms,
            `last_timestamp_ms` = :last_timestamp_ms
        ",
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_query() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let project_id = "project_id";
        let session1 = session::Session {
            id: "id1".to_string(),
            hash: None,
            meta: session::Meta {
                branch: None,
                commit: None,
                start_timestamp_ms: 1,
                last_timestamp_ms: 2,
            },
        };
        let session2 = session::Session {
            id: "id2".to_string(),
            hash: Some("hash2".to_string()),
            meta: session::Meta {
                branch: Some("branch2".to_string()),
                commit: Some("commit2".to_string()),
                start_timestamp_ms: 3,
                last_timestamp_ms: 4,
            },
        };
        let sessions = vec![&session1, &session2];

        database.insert(project_id, &sessions)?;

        assert_eq!(
            database.list_by_project_id(project_id, None)?,
            vec![session2.clone(), session1.clone()]
        );
        assert_eq!(database.get_by_id("id1")?.unwrap(), session1);
        assert_eq!(database.get_by_id("id2")?.unwrap(), session2);
        assert_eq!(database.get_by_id("id3")?, None);

        Ok(())
    }

    #[test]
    fn test_update() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let project_id = "project_id";
        let session1 = session::Session {
            id: "id1".to_string(),
            hash: None,
            meta: session::Meta {
                branch: None,
                commit: None,
                start_timestamp_ms: 1,
                last_timestamp_ms: 2,
            },
        };
        let session_updated = session::Session {
            id: "id1".to_string(),
            hash: Some("hash2".to_string()),
            meta: session::Meta {
                branch: Some("branch2".to_string()),
                commit: Some("commit2".to_string()),
                start_timestamp_ms: 3,
                last_timestamp_ms: 4,
            },
        };
        database.insert(project_id, &[&session1])?;
        database.insert(project_id, &[&session_updated])?;

        assert_eq!(
            database.list_by_project_id(project_id, None)?,
            vec![session_updated.clone()]
        );
        assert_eq!(database.get_by_id("id1")?.unwrap(), session_updated);

        Ok(())
    }
}
