use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{database, path::Normalize, projects::ProjectId, sessions::SessionId};

use super::{delta, operations};

#[derive(Clone)]
pub struct Database {
    database: database::Database,
}

impl Database {
    pub fn new(database: database::Database) -> Database {
        Database { database }
    }

    pub fn insert<P: AsRef<Path>>(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        file_path: P,
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
                    ":file_path": file_path.as_ref().normalize().as_path().display().to_string(),
                    ":timestamp_ms": timestamp_ms,
                    ":operations": operations,
                })
                .context("Failed to execute insert statement")?;
            }
            Ok(())
        })?;

        Ok(())
    }

    #[inline]
    pub fn list_by_project_id_session_id(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
    ) -> Result<HashMap<PathBuf, Vec<delta::Delta>>> {
        self.list_by_project_id_session_id_and_filter(project_id, session_id, Vec::<PathBuf>::new())
    }

    pub fn list_by_project_id_session_id_and_filter<P: AsRef<Path>, F: AsRef<[P]>>(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        file_path_filter: F,
    ) -> Result<HashMap<PathBuf, Vec<delta::Delta>>> {
        self.database.transaction(|tx| {
            let mut stmt = list_by_project_id_session_id_stmt(tx)
                .context("Failed to prepare query statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":project_id": project_id,
                    ":session_id": session_id,
                })
                .context("Failed to execute query statement")?;
            let mut deltas: HashMap<PathBuf, Vec<super::Delta>> = HashMap::new();
            while let Some(row) = rows
                .next()
                .context("Failed to iterate over query results")?
            {
                let file_path: String = row.get(0).context("Failed to get file_path")?;
                let file_path = PathBuf::from(file_path);

                {
                    let mut contains = false;
                    for filter in file_path_filter.as_ref() {
                        let filter = filter.as_ref();
                        if filter == file_path.as_path() {
                            contains = true;
                            break;
                        }
                    }
                    if !contains {
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

#[cfg(test)]
mod tests {
    use crate::tests;

    use super::*;

    #[test]
    fn insert_query() -> Result<()> {
        let db = tests::test_database();
        let database = Database::new(db);

        let project_id = ProjectId::generate();
        let session_id = SessionId::generate();
        let file_path = PathBuf::from("file_path");
        let delta1 = delta::Delta {
            timestamp_ms: 0,
            operations: vec![operations::Operation::Insert((0, "text".to_string()))],
        };
        let deltas = vec![delta1.clone()];

        database.insert(&project_id, &session_id, &file_path, &deltas)?;

        assert_eq!(
            database.list_by_project_id_session_id(&project_id, &session_id)?,
            vec![(file_path, vec![delta1])].into_iter().collect()
        );

        Ok(())
    }

    #[test]
    fn insert_update() -> Result<()> {
        let db = tests::test_database();
        let database = Database::new(db);

        let project_id = ProjectId::generate();
        let session_id = SessionId::generate();
        let file_path = PathBuf::from("file_path");
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

        database.insert(&project_id, &session_id, &file_path, &vec![delta1])?;
        database.insert(&project_id, &session_id, &file_path, &vec![delta2.clone()])?;

        assert_eq!(
            database.list_by_project_id_session_id(&project_id, &session_id)?,
            vec![(file_path, vec![delta2])].into_iter().collect()
        );

        Ok(())
    }

    #[test]
    fn aggregate_deltas_by_file() -> Result<()> {
        let db = tests::test_database();
        let database = Database::new(db);

        let project_id = ProjectId::generate();
        let session_id = SessionId::generate();
        let file_path1 = PathBuf::from("file_path1");
        let file_path2 = PathBuf::from("file_path2");
        let delta1 = delta::Delta {
            timestamp_ms: 1,
            operations: vec![operations::Operation::Insert((0, "text".to_string()))],
        };
        let delta2 = delta::Delta {
            timestamp_ms: 2,
            operations: vec![operations::Operation::Insert((
                0,
                "updated_text".to_string(),
            ))],
        };

        database.insert(&project_id, &session_id, &file_path1, &vec![delta1.clone()])?;
        database.insert(&project_id, &session_id, &file_path2, &vec![delta1.clone()])?;
        database.insert(&project_id, &session_id, &file_path2, &vec![delta2.clone()])?;

        assert_eq!(
            database.list_by_project_id_session_id(&project_id, &session_id)?,
            vec![
                (file_path1, vec![delta1.clone()]),
                (file_path2, vec![delta1, delta2])
            ]
            .into_iter()
            .collect()
        );

        Ok(())
    }
}
