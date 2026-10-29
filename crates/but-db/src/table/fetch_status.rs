#![allow(missing_docs)]

use rusqlite::OptionalExtension;

use crate::{DbHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20260715120000,
    SchemaVersion::Zero,
    "CREATE TABLE `fetch_status`(
	`singleton` INTEGER NOT NULL PRIMARY KEY CHECK (`singleton` = 1),
	`last_attempted_ms` INTEGER NOT NULL,
	`last_successful_ms` INTEGER,
	`last_error` TEXT
);",
)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchStatus {
    pub last_attempted_ms: i64,
    pub last_successful_ms: Option<i64>,
    pub last_error: Option<String>,
}

impl DbHandle {
    pub fn fetch_status(&self) -> FetchStatusHandle<'_> {
        FetchStatusHandle { conn: &self.conn }
    }

    pub fn fetch_status_mut(&mut self) -> FetchStatusHandleMut<'_> {
        FetchStatusHandleMut { conn: &self.conn }
    }
}

impl<'conn> Transaction<'conn> {
    pub fn fetch_status(&self) -> FetchStatusHandle<'_> {
        FetchStatusHandle { conn: self.inner() }
    }

    pub fn fetch_status_mut(&mut self) -> FetchStatusHandleMut<'_> {
        FetchStatusHandleMut { conn: self.inner() }
    }
}

pub struct FetchStatusHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct FetchStatusHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl FetchStatusHandle<'_> {
    pub fn get(&self) -> rusqlite::Result<Option<FetchStatus>> {
        self.conn
            .query_row(
                "SELECT last_attempted_ms, last_successful_ms, last_error
                 FROM fetch_status WHERE singleton = 1",
                [],
                |row| {
                    Ok(FetchStatus {
                        last_attempted_ms: row.get(0)?,
                        last_successful_ms: row.get(1)?,
                        last_error: row.get(2)?,
                    })
                },
            )
            .optional()
    }
}

impl FetchStatusHandleMut<'_> {
    /// Record an attempt that counts as successful. `error` carries failures that were
    /// tolerated (e.g. from remotes the caller does not depend on): the success timestamp
    /// still advances while the errors are kept for diagnostics.
    pub fn record_success(
        &mut self,
        attempted_ms: i64,
        error: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO fetch_status (
                singleton, last_attempted_ms, last_successful_ms, last_error
             ) VALUES (1, ?1, ?1, ?2)
             ON CONFLICT(singleton) DO UPDATE SET
                last_attempted_ms = excluded.last_attempted_ms,
                last_successful_ms = excluded.last_successful_ms,
                last_error = excluded.last_error",
            rusqlite::params![attempted_ms, error],
        )?;
        Ok(())
    }

    pub fn record_failure(&mut self, attempted_ms: i64, error: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO fetch_status (
                singleton, last_attempted_ms, last_successful_ms, last_error
             ) VALUES (1, ?1, NULL, ?2)
             ON CONFLICT(singleton) DO UPDATE SET
                last_attempted_ms = excluded.last_attempted_ms,
                last_error = excluded.last_error",
            rusqlite::params![attempted_ms, error],
        )?;
        Ok(())
    }
}
