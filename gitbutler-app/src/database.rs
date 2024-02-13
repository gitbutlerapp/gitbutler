use std::{fs, path, sync::Arc};

use anyhow::{Context, Result};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use refinery::config::Config;
use rusqlite::Transaction;
use tauri::{AppHandle, Manager};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/database/migrations");
}

#[derive(Clone)]
pub struct Database {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl TryFrom<&AppHandle> for Database {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(database) = value.try_state::<Database>() {
            Ok(database.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            fs::create_dir_all(&app_data_dir).context("failed to create local data dir")?;
            Self::open(app_data_dir.join("database.sqlite3"))
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

#[cfg(test)]
impl TryFrom<&path::PathBuf> for Database {
    type Error = anyhow::Error;

    fn try_from(value: &path::PathBuf) -> Result<Self, Self::Error> {
        Self::open(value.join("database.sqlite3"))
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
                    .for_each(|migration| tracing::info!(%migration, "migration applied"));
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
    fn smoke() {
        let data_dir = test_utils::temp_dir();
        let db = Database::try_from(&data_dir).unwrap();
        db.transaction(|tx| {
            tx.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
                .unwrap();
            tx.execute("INSERT INTO test (id) VALUES (1)", []).unwrap();
            let mut stmt = tx.prepare("SELECT id FROM test").unwrap();
            let mut rows = stmt.query([]).unwrap();
            let row = rows.next().unwrap().unwrap();
            let id: i32 = row.get(0).unwrap();
            assert_eq!(id, 1_i32);
            Ok(())
        })
        .unwrap();
    }
}
