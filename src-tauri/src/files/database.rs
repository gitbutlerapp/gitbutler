use std::collections::HashMap;

use anyhow::{Context, Result};
use sha1::{Digest, Sha1};

use crate::database;

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
        project_id: &str,
        session_id: &str,
        file_path: &str,
        content: &str,
    ) -> Result<()> {
        let mut hasher = Sha1::new();
        hasher.update(content);
        let sha1 = hasher.finalize();

        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = is_content_exist_by_sha1_stmt(tx)
                .context("Failed to prepare is_content_exist_by_sha1 statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":sha1": sha1.as_slice(),
                })
                .context("Failed to execute is_content_exist_by_sha1 statement")?;
            let is_content_exist: bool = rows
                .next()
                .context("Failed to iterate over is_content_exist_by_sha1 results")?
                .is_some();

            if !is_content_exist {
                let mut stmt =
                    insert_content_stmt(tx).context("Failed to prepare insert statement")?;
                stmt.execute(rusqlite::named_params! {
                    ":sha1": sha1.as_slice(),
                    ":content": content,
                })
                .context("Failed to execute insert statement")?;
            }

            let mut stmt =
                insert_file_stmt(tx).context("Failed to prepare insert file statement")?;
            stmt.execute(rusqlite::named_params! {
                ":project_id": project_id,
                ":session_id": session_id,
                ":file_path": file_path,
                ":sha1": sha1.as_slice(),
            })
            .context("Failed to execute insert statement")?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn list_by_project_id_session_id(
        &self,
        project_id: &str,
        session_id: &str,
        file_path_filter: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        self.database
            .transaction(|tx| -> Result<HashMap<String, String>> {
                let mut stmt = list_by_project_id_session_id_stmt(tx)
                    .context("Failed to prepare list_by_session_id statement")?;
                let mut rows = stmt
                    .query(rusqlite::named_params! {
                        ":project_id": project_id,
                        ":session_id": session_id,
                    })
                    .context("Failed to execute list_by_session_id statement")?;

                let mut files = HashMap::new();
                while let Some(row) = rows
                    .next()
                    .context("Failed to iterate over list_by_session_id results")?
                {
                    let file_path: String = row.get(0)?;
                    if let Some(file_path_filter) = &file_path_filter {
                        if !file_path_filter.contains(&file_path.as_str()) {
                            continue;
                        }
                    }

                    let content: String = row.get(1)?;
                    files.insert(file_path, content);
                }
                Ok(files)
            })
    }
}

fn list_by_project_id_session_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `file_path`, `content`
        FROM `files`
        JOIN `contents` ON `files`.`sha1` = `contents`.`sha1`
        WHERE `project_id` = :project_id AND `session_id` = :session_id",
    )?)
}

fn is_content_exist_by_sha1_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT 1
            FROM `contents`
            WHERE `sha1` = :sha1",
    )?)
}

fn insert_content_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "INSERT INTO `contents` (
            `sha1`, `content`
        ) VALUES (
            :sha1, :content
        )",
    )?)
}

fn insert_file_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "INSERT INTO `files` (
            `project_id`, `session_id`, `file_path`, `sha1`
        ) VALUES (
            :project_id, :session_id, :file_path, :sha1
        ) ON CONFLICT(`project_id`, `session_id`, `file_path`) 
            DO UPDATE SET `sha1` = :sha1",
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_query_with_filter() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let project_id = "project_id";
        let session_id = "session_id";
        let file_path = "file_path";

        let file = "file";
        database
            .insert(project_id, session_id, file_path, file)
            .context("Failed to insert file")?;

        assert_eq!(
            database
                .list_by_project_id_session_id(project_id, session_id, Some(vec!["file_path"]))
                .context("filed to list by session id")?,
            {
                let mut files = HashMap::new();
                files.insert(String::from(file_path), file.to_string());
                files
            }
        );
        assert_eq!(
            database
                .list_by_project_id_session_id(project_id, session_id, Some(vec!["file_path2"]))
                .context("filed to list by session id")?,
            HashMap::new()
        );

        Ok(())
    }

    #[test]
    fn test_upsert() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let project_id = "project_id";
        let session_id = "session_id";
        let file_path = "file_path";

        let file = "file";
        database
            .insert(project_id, session_id, file_path, file)
            .context("Failed to insert file1")?;

        let file2 = "file2";
        database
            .insert(project_id, session_id, file_path, file2)
            .context("Failed to insert file2")?;

        assert_eq!(
            database
                .list_by_project_id_session_id(project_id, session_id, None)
                .context("filed to list by session id")?,
            {
                let mut files = HashMap::new();
                files.insert(String::from(file_path), file2.to_string());
                files
            }
        );

        Ok(())
    }
}
