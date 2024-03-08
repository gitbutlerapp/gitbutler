use std::{path, sync::Arc};

use anyhow::{Context, Result};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use refinery::config::Config;
use rusqlite::Transaction;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/database/migrations");
}

#[derive(Clone)]
pub struct Database {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl Database {
    pub fn open_in_directory<P: AsRef<path::Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf().join("database.sqlite3");
        let manager = SqliteConnectionManager::file(&path);
        let pool = r2d2::Pool::new(manager)?;
        let mut cfg = Config::new(refinery::config::ConfigDbType::Sqlite)
            .set_db_path(path.as_path().to_str().unwrap());
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
    use crate::tests;

    use super::*;

    #[test]
    fn smoke() {
        let data_dir = tests::temp_dir();
        let db = Database::open_in_directory(data_dir).unwrap();
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
