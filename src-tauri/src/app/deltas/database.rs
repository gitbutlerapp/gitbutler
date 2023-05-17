use std::collections::HashMap;

use anyhow::{Context, Result};
use rusqlite::hooks;

use crate::database;

use super::{delta, operations};

#[derive(Clone)]
pub struct Database {
    database: database::Database,
}

impl Database {
    pub fn new(database: database::Database) -> Self {
        Self { database }
    }

    pub fn insert(
        &self,
        session_id: &str,
        file_path: &str,
        deltas: &Vec<delta::Delta>,
    ) -> Result<()> {
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            for delta in deltas {
                let operations = serde_json::to_vec(&delta.operations)
                    .context("Failed to serialize operations")?;
                let timestamp_ms = delta.timestamp_ms.to_string();
                stmt.execute(rusqlite::named_params! {
                    ":session_id": session_id,
                    ":file_path": file_path,
                    ":timestamp_ms": timestamp_ms,
                    ":operations": operations,
                })
                .context("Failed to execute insert statement")?;
            }
            Ok(())
        })?;

        log::info!(
            "db: inserted {} deltas for file {} for session {}",
            deltas.len(),
            file_path,
            session_id
        );

        Ok(())
    }

    pub fn get_by_session_id_file_path(
        &self,
        session_id: &str,
        file_path: &str,
    ) -> Result<Option<delta::Delta>> {
        let mut delta: Option<delta::Delta> = None;
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = get_by_session_id_file_path_stmt(tx)
                .context("Failed to prepare get_by_session_id_file_path statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":session_id": session_id,
                    ":file_path": file_path,
                })
                .context("Failed to execute get_by_session_id_file_path statement")?;
            while let Some(row) = rows
                .next()
                .context("Failed to iterate over get_by_session_id_file_path results")?
            {
                let timestamp_ms: String = row.get(0).context("Failed to get timestamp_ms")?;
                let operations: Vec<u8> = row.get(1).context("Failed to get operations")?;
                let operations: Vec<operations::Operation> = serde_json::from_slice(&operations)
                    .context("Failed to deserialize operations")?;
                let timestamp_ms: u128 = timestamp_ms
                    .parse()
                    .context("Failed to parse timestamp_ms")?;
                delta = Some(delta::Delta {
                    timestamp_ms,
                    operations,
                });
            }
            Ok(())
        })?;
        Ok(delta)
    }

    pub fn on<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(&str, &str, delta::Delta) + Send + 'static,
    {
        let boxed_database = Box::new(self.database.clone());
        self.database.on_update(
            move |action, _database_name, table_name, rowid| match action {
                hooks::Action::SQLITE_INSERT | hooks::Action::SQLITE_UPDATE => match table_name {
                    "deltas" => {
                        if let Err(err) = boxed_database.transaction(|tx| -> Result<()> {
                            let mut stmt = get_by_rowid_stmt(tx)
                                .context("Failed to prepare get_by_rowid statement")?;
                            let mut rows = stmt
                                .query(rusqlite::named_params! {
                                    ":rowid": rowid,
                                })
                                .context("Failed to execute get_by_rowid statement")?;

                            while let Some(row) = rows
                                .next()
                                .context("Failed to iterate over get_by_rowid results")?
                            {
                                let session_id: String =
                                    row.get(0).context("Failed to get session_id")?;
                                let file_path: String =
                                    row.get(1).context("Failed to get file_path")?;
                                let timestamp_ms: String =
                                    row.get(2).context("Failed to get timestamp_ms")?;
                                let operations: Vec<u8> =
                                    row.get(3).context("Failed to get operations")?;
                                let operations: Vec<operations::Operation> =
                                    serde_json::from_slice(&operations)
                                        .context("Failed to deserialize operations")?;
                                let timestamp_ms: u128 = timestamp_ms
                                    .parse()
                                    .context("Failed to parse timestamp_ms")?;
                                let delta = delta::Delta {
                                    timestamp_ms,
                                    operations,
                                };
                                callback(&session_id, &file_path, delta);
                            }

                            Ok(())
                        }) {
                            log::error!("db: failed to get delta by rowid: {}", err);
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
        )
    }

    pub fn list_by_session_id(
        &self,
        session_id: &str,
        file_path_filter: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<delta::Delta>>> {
        let mut deltas = HashMap::new();
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt =
                list_by_session_id_stmt(tx).context("Failed to prepare query statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":session_id": session_id,
                })
                .context("Failed to execute query statement")?;
            while let Some(row) = rows
                .next()
                .context("Failed to iterate over query results")?
            {
                let file_path: String = row.get(0).context("Failed to get file_path")?;
                if let Some(file_path_filter) = &file_path_filter {
                    if !file_path_filter.contains(&file_path.as_str()) {
                        continue;
                    }
                }
                let timestamp_ms: String = row.get(1).context("Failed to get timestamp_ms")?;
                let operations: Vec<u8> = row.get(2).context("Failed to get operations")?;
                let operations: Vec<operations::Operation> = serde_json::from_slice(&operations)
                    .context("Failed to deserialize operations")?;
                let timestamp_ms: u128 = timestamp_ms
                    .parse()
                    .context("Failed to parse timestamp_ms as u64")?;
                let delta = delta::Delta {
                    timestamp_ms,
                    operations,
                };
                deltas.extend(vec![(file_path, vec![delta])]);
            }
            Ok(())
        })?;
        Ok(deltas)
    }
}

fn get_by_session_id_file_path_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `timestamp_ms`, `operations` FROM `deltas` WHERE `session_id` = :session_id AND `file_path` = :file_path",
    )?)
}

fn get_by_rowid_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `session_id`, `timestamp_ms`, `operations`, `file_path` FROM `deltas` WHERE `rowid` = :rowid",
    )?)
}

fn list_by_session_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `file_path`, `timestamp_ms`, `operations` FROM `deltas` WHERE `session_id` = :session_id",
    )?)
}

fn insert_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "INSERT INTO `deltas` (
            `session_id`, `timestamp_ms`, `operations`, `file_path`
        ) VALUES (
            :session_id, :timestamp_ms, :operations, :file_path
        )
        ON CONFLICT(`session_id`, `file_path`, `timestamp_ms`) DO UPDATE SET
            `operations` = :operations
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

        let session_id = "session_id";
        let file_path = "file_path";
        let delta1 = delta::Delta {
            timestamp_ms: 0,
            operations: vec![operations::Operation::Insert((0, "text".to_string()))],
        };
        let deltas = vec![delta1.clone()];

        database.insert(session_id, file_path, &deltas)?;

        assert_eq!(
            database.list_by_session_id(session_id, None)?,
            vec![(file_path.to_string(), vec![delta1.clone()])]
                .into_iter()
                .collect()
        );

        assert_eq!(
            database.get_by_session_id_file_path(session_id, file_path)?,
            Some(delta1)
        );

        assert_eq!(
            database.get_by_session_id_file_path(session_id, "other_file_path")?,
            None
        );

        Ok(())
    }

    #[test]
    fn test_insert_update() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let session_id = "session_id";
        let file_path = "file_path";
        let delta1 = delta::Delta {
            timestamp_ms: 0,
            operations: vec![operations::Operation::Insert((0, "text".to_string()))],
        };
        let delta2 = delta::Delta {
            timestamp_ms: 0,
            operations: vec![operations::Operation::Insert((
                0,
                "updated_text".to_string(),
            ))],
        };

        database.insert(session_id, file_path, &vec![delta1])?;
        database.insert(session_id, file_path, &vec![delta2.clone()])?;

        assert_eq!(
            database.list_by_session_id(session_id, None)?,
            vec![(file_path.to_string(), vec![delta2.clone()])]
                .into_iter()
                .collect()
        );

        assert_eq!(
            database.get_by_session_id_file_path(session_id, file_path)?,
            Some(delta2)
        );

        Ok(())
    }
}
