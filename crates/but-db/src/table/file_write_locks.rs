use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20250704130757,
    "CREATE TABLE `file_write_locks`(
	`path` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`owner` TEXT NOT NULL
);",
)];

/// Tests are in `but-db/tests/db/table/file_write_lock.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileWriteLock {
    pub path: String,
    pub created_at: chrono::NaiveDateTime,
    pub owner: String,
}

impl DbHandle {
    pub fn file_write_locks(&self) -> FileWriteLocksHandle<'_> {
        FileWriteLocksHandle { conn: &self.conn }
    }

    pub fn file_write_locks_mut(&mut self) -> FileWriteLocksHandleMut<'_> {
        FileWriteLocksHandleMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn file_write_locks(&self) -> FileWriteLocksHandle<'_> {
        FileWriteLocksHandle { conn: self.inner() }
    }

    pub fn file_write_locks_mut(&mut self) -> FileWriteLocksHandleMut<'_> {
        FileWriteLocksHandleMut { conn: self.inner() }
    }
}

pub struct FileWriteLocksHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct FileWriteLocksHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl FileWriteLocksHandle<'_> {
    /// Lists all file write locks.
    pub fn list(&self) -> rusqlite::Result<Vec<FileWriteLock>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, created_at, owner FROM file_write_locks")?;

        let results = stmt.query_map([], |row| {
            Ok(FileWriteLock {
                path: row.get(0)?,
                created_at: row.get(1)?,
                owner: row.get(2)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }
}

impl FileWriteLocksHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> FileWriteLocksHandle<'_> {
        FileWriteLocksHandle { conn: self.conn }
    }

    /// Inserts or replaces a file write lock.
    pub fn insert(&mut self, lock: FileWriteLock) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO file_write_locks (path, created_at, owner) VALUES (?1, ?2, ?3)",
            rusqlite::params![lock.path, lock.created_at, lock.owner],
        )?;
        Ok(())
    }

    /// Deletes a file write lock by path.
    pub fn delete(&mut self, path: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM file_write_locks WHERE path = ?1", [path])?;
        Ok(())
    }
}
