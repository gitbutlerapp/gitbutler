use std::{fs, path, sync::Arc};

use anyhow::{Context, Result};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use refinery::config::Config;
use rusqlite::Transaction;
use tauri::AppHandle;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/database/migrations");
}

#[derive(Clone)]
pub struct Database {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl TryFrom<&path::PathBuf> for Database {
    type Error = anyhow::Error;

    fn try_from(path: &path::PathBuf) -> Result<Self, Self::Error> {
        Self::open(path)
    }
}

impl TryFrom<&AppHandle> for Database {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get local data dir"))?;
        fs::create_dir_all(&local_data_dir).context("Failed to create local data dir")?;
        Self::try_from(&local_data_dir.join("database.sqlite3"))
    }
}

impl Database {
    fn open<P: AsRef<path::Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let manager = SqliteConnectionManager::file(path);
        let pool = r2d2::Pool::new(manager)?;
        let mut cfg =
            Config::new(refinery::config::ConfigDbType::Sqlite).set_db_path(path.to_str().unwrap());
        embedded::migrations::runner()
            .run(&mut cfg)
            .map(|report| {
                report
                    .applied_migrations()
                    .iter()
                    .for_each(|migration| tracing::info!(%migration, "migration applied"))
            })
            .context("Failed to run migrations")?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    pub fn transaction<T>(&self, f: impl FnOnce(&Transaction) -> Result<T>) -> Result<T> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction().context("Failed to start transaction")?;
        let result = f(&tx)?;
        tx.commit().context("Failed to commit transaction")?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;
    #[test]
    fn test_memory() {
        let db = Database::try_from(&test_utils::temp_dir().join("test.db")).unwrap();
        db.transaction(|tx| {
            tx.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
                .unwrap();
            tx.execute("INSERT INTO test (id) VALUES (1)", []).unwrap();
            let mut stmt = tx.prepare("SELECT id FROM test").unwrap();
            let mut rows = stmt.query([]).unwrap();
            let row = rows.next().unwrap().unwrap();
            let id: i32 = row.get(0).unwrap();
            assert_eq!(id, 1);
            Ok(())
        })
        .unwrap();
    }
}
