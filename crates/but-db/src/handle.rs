use crate::{DbHandle, FILE_NAME, migration};
use diesel::connection::SimpleConnection;
use diesel::{Connection, SqliteConnection};
use std::path::{Path, PathBuf};
use tracing::instrument;

impl std::fmt::Debug for DbHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbHandle").field("db", &self.url).finish()
    }
}

/// A handle to the database connection.
impl DbHandle {
    /// Create a new instance connecting to a file-based database contained in `db_dir`.
    /// It will be created or updated automatically.
    pub fn new_in_directory(db_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut db_dir = db_dir.as_ref().to_owned();
        let cwd = std::env::current_dir()?;
        if db_dir.is_relative() {
            db_dir = cwd.join(db_dir);
        }
        let db_file_path = Self::db_file_path(&db_dir);
        if let Some(parent_dir_to_create) = db_file_path.parent().filter(|dir| !dir.exists()) {
            std::fs::create_dir_all(parent_dir_to_create)?;
        }
        let db_file_path = db_file_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in database file path"))?;
        Self::new_at_url(db_file_path)
    }

    /// A new instance connecting to the database at the given `url`.
    #[instrument(level = "debug", skip(url), err(Debug))]
    pub fn new_at_url(url: impl Into<String>) -> anyhow::Result<Self> {
        let url = url.into();
        let mut conn = SqliteConnection::establish(&url)?;
        improve_concurrency(&mut conn)?;
        let rsconn = run_migrations(&url)?;
        Ok(DbHandle { conn, rsconn, url })
    }

    /// Return the path to the standard database file.
    pub fn db_file_path(db_dir: impl AsRef<Path>) -> PathBuf {
        db_dir.as_ref().join(FILE_NAME)
    }
}

/// Improve parallelism and make it non-fatal.
/// Blanket setting from https://github.com/the-lean-crate/criner/discussions/5, maybe needs tuning.
/// Also, it's a known issue, maybe order matters?
/// https://github.com/diesel-rs/diesel/issues/2365#issuecomment-2899347817
/// TODO: the busy_timeout doesn't seem to be effective.
fn improve_concurrency(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    // For safety, execute them one by one. Otherwise, they can individually fail, silently (at least the `busy_timeout`.
    for query in [
        "PRAGMA busy_timeout = 30000;        -- wait X milliseconds, but not all at once, before for timing out with error.",
        "PRAGMA journal_mode = WAL;          -- better write-concurrency",
        "PRAGMA synchronous = NORMAL;        -- fsync only in critical moments",
        "PRAGMA wal_autocheckpoint = 1000;   -- write WAL changes back every 1000 pages, for an in average 1MB WAL file.",
        "PRAGMA wal_checkpoint(TRUNCATE);    -- free some space by truncating possibly massive WAL files from the last run.",
    ] {
        conn.batch_execute(query)?;
    }
    Ok(())
}

fn run_migrations(url: &str) -> anyhow::Result<rusqlite::Connection> {
    let mut db = rusqlite::Connection::open(url)?;
    let policy = backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(std::time::Duration::from_millis(500)))
        .build();

    backoff::retry(policy, || {
        let count = migration::run(&mut db, migration::ours())?;
        if count > 0 {
            tracing::info!("Database updated with {count} migrations");
        }
        Ok::<_, backoff::Error<migration::Error>>(())
    })?;
    Ok(db)
}
