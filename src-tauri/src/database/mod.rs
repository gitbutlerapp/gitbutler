use std::path;

use anyhow::{Context, Result};

use rusqlite::{hooks, Transaction};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/database/migrations");
}

#[derive(Clone)]
pub struct Database {
    pub pool: r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
}

impl Database {
    #[cfg(test)]
    pub fn memory() -> Result<Self> {
        let manager = r2d2_sqlite::SqliteConnectionManager::memory().with_init(|conn| {
            embedded::migrations::runner()
                .run(conn)
                .map(|report| {
                    report
                        .applied_migrations()
                        .iter()
                        .for_each(|m| log::info!("Applied migration: {}", m))
                })
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        });
        Ok(Self {
            pool: r2d2::Pool::new(manager).context("Failed to create pool")?,
        })
    }

    pub fn open<P: AsRef<path::Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let manager = r2d2_sqlite::SqliteConnectionManager::file(path).with_init(|conn| {
            embedded::migrations::runner()
                .run(conn)
                .map(|report| {
                    report
                        .applied_migrations()
                        .iter()
                        .for_each(|m| log::info!("Applied migration: {}", m))
                })
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        });
        Ok(Self {
            pool: r2d2::Pool::new(manager).context("Failed to create pool")?,
        })
    }

    pub fn transaction<T>(&self, f: impl FnOnce(&Transaction) -> Result<T>) -> Result<T> {
        let mut conn = self
            .pool
            .get()
            .context("Failed to get connection from pool")?;
        let tx = conn.transaction().context("Failed to start transaction")?;
        let result = f(&tx)?;
        tx.commit().context("Failed to commit transaction")?;
        Ok(result)
    }

    pub fn on_update<F>(&self, hook: F) -> Result<()>
    where
        F: FnMut(hooks::Action, &str, &str, i64) + Send + 'static,
    {
        let conn = self
            .pool
            .get()
            .context("Failed to get connection from pool")?;
        conn.update_hook(Some(hook));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_memory() {
        let db = Database::memory().unwrap();
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
