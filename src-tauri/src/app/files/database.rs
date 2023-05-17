use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::database;

#[derive(Clone)]
pub struct Database {
    database: database::Database,
}

impl Database {
    pub fn new(database: database::Database) -> Self {
        Self { database }
    }

    pub fn insert(&self, session_id: &str, file_path: &str, content: &str) -> Result<()> {
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = insert_stmt(tx).context("Failed to prepare insert statement")?;
            stmt.execute(rusqlite::named_params! {
                ":session_id": session_id,
                ":file_path": file_path,
                ":content": content,
            })
            .context("Failed to execute insert statement")?;
            Ok(())
        })?;
        log::info!("db: inserted file {} for session {}", file_path, session_id);
        Ok(())
    }

    pub fn list_by_session_id(
        &self,
        session_id: &str,
        file_path_filter: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        let mut files = HashMap::new();
        self.database.transaction(|tx| -> Result<()> {
            let mut stmt = list_by_session_id_stmt(tx)
                .context("Failed to prepare list_by_session_id statement")?;
            let mut rows = stmt
                .query(rusqlite::named_params! {
                    ":session_id": session_id,
                })
                .context("Failed to execute list_by_session_id statement")?;
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
            Ok(())
        })?;
        Ok(files)
    }

    pub fn on<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(&str, &str, &str) + Send + 'static,
    {
        let boxed_database = Box::new(self.database.clone());
        self.database.on_update(
            move |action, _database_name, table_name, rowid| match action {
                rusqlite::hooks::Action::SQLITE_INSERT | rusqlite::hooks::Action::SQLITE_UPDATE => {
                    match table_name {
                        "files" => {
                            if let Err(err) = boxed_database.transaction(|tx| -> Result<()> {
                                let mut stmt = get_by_rowid_stmt(tx)
                                    .context("Failed to prepare get_by_rowid statement")?;

                                let mut rows = stmt
                                    .query(rusqlite::named_params! {
                                        ":rowid": rowid,
                                    })
                                    .context("Failed to execute get_by_rowid statement")?;

                                if let Some(row) = rows
                                    .next()
                                    .context("Failed to iterate over get_by_rowid results")?
                                {
                                    let file_path: String = row.get(0)?;
                                    let content: String = row.get(1)?;
                                    let session_id: String = row.get(2)?;
                                    callback(&session_id, &file_path, &content)
                                }

                                Ok(())
                            }) {
                                log::error!("db: failed to get file by rowid: {}", err);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
        )
    }
}

fn list_by_session_id_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `file_path`, `content`
        FROM `files`
        WHERE `session_id` = :session_id",
    )?)
}

fn insert_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "INSERT INTO `files` (
            `session_id`, `file_path`, `content`
        ) VALUES (
            :session_id, :file_path, :content
        )
        ON CONFLICT(`session_id`, `file_path`) DO UPDATE SET
            `content` = :content
        ",
    )?)
}

fn get_by_rowid_stmt<'conn>(
    tx: &'conn rusqlite::Transaction,
) -> Result<rusqlite::CachedStatement<'conn>> {
    Ok(tx.prepare_cached(
        "SELECT `file_path`, `content`, `session_id`
        FROM `files`
        WHERE `rowid` = :rowid",
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_query_with_filter() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let session_id = "session_id";
        let file_path = "file_path";

        let file = "file";
        database
            .insert(session_id, file_path, file)
            .context("Failed to insert file")?;

        assert_eq!(
            database
                .list_by_session_id(session_id, Some(vec!["file_path"]))
                .context("filed to list by session id")?,
            {
                let mut files = HashMap::new();
                files.insert(String::from(file_path), file.to_string());
                files
            }
        );
        assert_eq!(
            database
                .list_by_session_id(session_id, Some(vec!["file_path2"]))
                .context("filed to list by session id")?,
            HashMap::new()
        );

        Ok(())
    }

    #[test]
    fn test_upsert() -> Result<()> {
        let db = database::Database::memory()?;
        let database = Database::new(db);

        let session_id = "session_id";
        let file_path = "file_path";

        let file = "file";
        database
            .insert(session_id, file_path, file)
            .context("Failed to insert file1")?;

        let file2 = "file2";
        database
            .insert(session_id, file_path, file2)
            .context("Failed to insert file2")?;

        assert_eq!(
            database
                .list_by_session_id(session_id, None)
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
