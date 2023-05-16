use std::{path, sync::Mutex};

use anyhow::{Context, Result};

use rusqlite::{Connection, Transaction};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/database/migrations");
}

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    #[cfg(test)]
    pub fn memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory().context("Failed to open in memory database")?;

        embedded::migrations::runner()
            .run(&mut conn)
            .context("Failed to run migrations")?;
        Ok(Self { conn: conn.into() })
    }

    pub fn open<P: AsRef<path::Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let mut conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at path: {}", path.display()))?;
        embedded::migrations::runner()
            .run(&mut conn)
            .context("Failed to run migrations")?;
        Ok(Self { conn: conn.into() })
    }

    pub fn transaction(&self, f: impl FnOnce(&Transaction) -> Result<()>) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().context("Failed to start transaction")?;
        f(&tx)?;
        tx.commit().context("Failed to commit transaction")?;
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
