use std::{collections::HashMap, path};

use anyhow::{Context, Result};

use super::{delta, operations};
use crate::{database, projects::ProjectId, sessions::SessionId};

#[derive(Clone)]
pub struct Database {
    database: database::Database,
}

impl Database {
    pub fn new(database: database::Database) -> Database {
        Database { database }
    }

    pub fn insert(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        file_path: &path::Path,
        deltas: &Vec<delta::Delta>,
    ) -> Result<()> {
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            for delta in deltas {
                let operations = serde_json::to_vec(&delta.operations)
                    .context("Failed to serialize operations")?;
                let timestamp_ms = delta.timestamp_ms.to_string();
                stmt.execute(rusqlite::named_params! {
                    ":project_id": project_id,
                    ":session_id": session_id,
                    ":file_path": file_path.display().to_string(),
                    ":timestamp_ms": timestamp_ms,
                    ":operations": operations,
                })
                .context("Failed to execute insert statement")?;
            }
            Ok(())
        })?;

        Ok(())
    }

    pub fn list_by_project_id_session_id(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        file_path_filter: &Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<delta::Delta>>> {
        self.database
            .transaction(|tx| -> Result<HashMap<String, Vec<delta::Delta>>> {
                let mut stmt = list_by_project_id_session_id_stmt(tx)
                    .context("Failed to prepare query statement")?;
                let mut rows = stmt
                    .query(rusqlite::named_params! {
                        ":project_id": project_id,
                        ":session_id": session_id,
                    })
                    .context("Failed to execute query statement")?;
                let mut deltas: HashMap<String, Vec<super::Delta>> = HashMap::new();
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
                    let operations: Vec<operations::Operation> =
                        serde_json::from_slice(&operations)
                            .context("Failed to deserialize operations")?;
                    let timestamp_ms: u128 = timestamp_ms
                        .parse()
                        .context("Failed to parse timestamp_ms as u64")?;
                    let delta = delta::Delta {
                        operations,
                        timestamp_ms,
                    };
                    if let Some(deltas_for_file_path) = deltas.get_mut(&file_path) {
                        deltas_for_file_path.push(delta);
                    } else {
                        deltas.insert(file_path, vec![delta]);
                    }
                }
                Ok(deltas)
            })
    }
}

fn list_by_project_id_session_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "
        SELECT `file_path`, `timestamp_ms`, `operations`
        FROM `deltas`
        WHERE `session_id` = :session_id AND `project_id` = :project_id
        ORDER BY `timestamp_ms` ASC",
    )?)
}

fn insert_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "INSERT INTO `deltas` (
            `project_id`, `session_id`, `timestamp_ms`, `operations`, `file_path`
        ) VALUES (
            :project_id, :session_id, :timestamp_ms, :operations, :file_path
        )
        ON CONFLICT(`project_id`, `session_id`, `file_path`, `timestamp_ms`) DO UPDATE SET
            `operations` = :operations
        ",
    )?)
}
