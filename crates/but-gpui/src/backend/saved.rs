//! Where the app keeps what it remembers between launches: a SQLite file per channel with a
//! JSON value per key. Losing it only resets what it held, so a file SQLite can't read is
//! replaced, and with none that opens the app runs on and saves nothing.
//!
//! Unlike the rest of the backend, the app opens and reads it on the UI thread as it starts,
//! and writes it there on quit: it's a few small rows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use parking_lot::Mutex;
use rusqlite::{Connection, ErrorCode, TransactionBehavior};

pub struct Database(Option<Mutex<Written>>);

/// The open database, with what this instance last wrote to it.
struct Written {
    conn: Connection,
    /// Each key's value as last written or loaded. Only keys whose value differs are written,
    /// so another instance's changes to the rest, or a newer build's values this one can't
    /// read, stay.
    rows: HashMap<String, String>,
    /// The newest state written, so an older one whose write comes late is dropped.
    version: u64,
}

impl Database {
    /// The channel's, in a folder of its own so deleting the folder resets the app with
    /// SQLite's side files.
    pub fn open() -> Self {
        Self::open_in(
            but_path::app_data_dir()
                .ok()
                .map(|dir| dir.join("gpui-state")),
        )
    }

    fn open_in(dir: Option<PathBuf>) -> Self {
        let Some(path) = dir.map(|dir| dir.join("state.sqlite")) else {
            return Self(None);
        };
        let opened = open_file(&path).or_else(|err| {
            if !unreadable(&err) {
                return Err(err);
            }
            tracing::warn!(
                "replacing {}, which SQLite can't read: {err}",
                path.display()
            );
            // With its side files, which SQLite would otherwise read into the new one.
            for side in ["", "-wal", "-shm"] {
                let mut file = path.clone().into_os_string();
                file.push(side);
                std::fs::remove_file(file).ok();
            }
            open_file(&path)
        });
        match opened {
            Ok(conn) => Self(Some(Mutex::new(Written {
                conn,
                rows: HashMap::new(),
                version: 0,
            }))),
            Err(err) => {
                tracing::warn!(
                    "not saving the app's state, as {} won't open: {err}",
                    path.display()
                );
                Self(None)
            }
        }
    }

    /// Every key with its value.
    pub fn read(&self) -> Vec<(String, String)> {
        let Some(written) = &self.0 else {
            return Vec::new();
        };
        let written = written.lock();
        let rows = written
            .conn
            .prepare("SELECT key, value FROM state")
            .and_then(|mut rows| {
                rows.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect()
            });
        rows.unwrap_or_else(|err| {
            tracing::warn!("couldn't read the app's state: {err}");
            Vec::new()
        })
    }

    /// Count `rows` as written, as the state just loaded is.
    pub fn loaded(&self, rows: Vec<(String, String)>) {
        if let Some(written) = &self.0 {
            written.lock().rows.extend(rows);
        }
    }

    /// Write the keys of the app's state `version` whose values changed since last written,
    /// unless a newer version was written already. A write that fails is left for the next.
    pub fn write(&self, version: u64, rows: Vec<(String, String)>) {
        let Some(written) = &self.0 else {
            return;
        };
        let mut written = written.lock();
        if version <= written.version {
            return;
        }
        let changed: Vec<_> = (rows.into_iter())
            .filter(|(key, value)| written.rows.get(key) != Some(value))
            .collect();
        if !changed.is_empty()
            && let Err(err) = write_rows(&mut written.conn, &changed)
        {
            tracing::warn!("couldn't save the app's state: {err}");
            return;
        }
        written.version = version;
        written.rows.extend(changed);
    }
}

fn write_rows(conn: &mut Connection, rows: &[(String, String)]) -> rusqlite::Result<()> {
    // Immediate, so another instance writing makes it wait rather than fail: a deferred
    // transaction that has to take the lock later fails at once.
    let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for (key, value) in rows {
        transaction.execute(
            "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
            (key, value),
        )?;
    }
    transaction.commit()
}

fn open_file(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent() {
        // Not being able to is the open's error to report.
        std::fs::create_dir_all(dir).ok();
    }
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    // WAL so two instances can share it, and NORMAL as losing the last writes to a power cut
    // is fine here.
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         CREATE TABLE IF NOT EXISTS state (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    Ok(conn)
}

/// SQLite can't read the file as a database, so replacing it loses nothing more.
fn unreadable(err: &rusqlite::Error) -> bool {
    (err.sqlite_error_code())
        .is_some_and(|code| matches!(code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt))
}

#[cfg(test)]
mod tests {
    use super::Database;

    fn rows(db: &Database) -> Vec<(String, String)> {
        let mut rows = db.read();
        rows.sort();
        rows
    }

    fn row(key: &str, value: &str) -> (String, String) {
        (key.into(), value.into())
    }

    #[test]
    fn what_is_written_is_read_back_after_reopening() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let db = Database::open_in(Some(dir.path().into()));
        db.write(1, vec![row("widths", "{}"), row("selection", "\"a\"")]);
        db.write(2, vec![row("widths", "{}"), row("selection", "\"b\"")]);
        drop(db);

        let db = Database::open_in(Some(dir.path().into()));
        assert_eq!(
            rows(&db),
            [row("selection", "\"b\""), row("widths", "{}")],
            "each key keeps its last value"
        );
        Ok(())
    }

    #[test]
    fn an_older_state_written_late_is_dropped() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let db = Database::open_in(Some(dir.path().into()));
        db.write(2, vec![row("selection", "\"new\"")]);
        db.write(1, vec![row("selection", "\"old\"")]);
        assert_eq!(
            rows(&db),
            [row("selection", "\"new\"")],
            "a quit's write isn't undone by a background one that started before it"
        );
        Ok(())
    }

    #[test]
    fn keys_this_instance_didnt_change_keep_another_instances_values() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let first = Database::open_in(Some(dir.path().into()));
        first.loaded(vec![row("widths", "{}"), row("selection", "\"a\"")]);
        let second = Database::open_in(Some(dir.path().into()));
        second.write(1, vec![row("widths", r#"{"Navigator":420}"#)]);

        first.write(1, vec![row("widths", "{}"), row("selection", "\"b\"")]);
        assert_eq!(
            rows(&first),
            [
                row("selection", "\"b\""),
                row("widths", r#"{"Navigator":420}"#)
            ],
            "only the key the first one changed is written"
        );
        Ok(())
    }

    #[test]
    fn a_file_sqlite_cant_read_is_replaced() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("state.sqlite"), "not a database")?;

        let db = Database::open_in(Some(dir.path().into()));
        db.write(1, vec![row("widths", "{}")]);
        assert_eq!(
            rows(&db),
            [row("widths", "{}")],
            "it saves again, in a new file"
        );
        Ok(())
    }

    #[test]
    fn without_a_file_that_opens_nothing_is_saved() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("a-file");
        std::fs::write(&file, "")?;

        // A folder that can't be made, as a file has its name.
        let db = Database::open_in(Some(file.join("gpui-state")));
        db.write(1, vec![row("widths", "{}")]);
        assert!(rows(&db).is_empty(), "the app runs on, saving nothing");
        Ok(())
    }
}
