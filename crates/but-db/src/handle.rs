use crate::migration::improve_concurrency;
use crate::{DbHandle, migration};
use std::path::{Path, PathBuf};
use tracing::instrument;

const FILE_NAME: &str = "but.sqlite";

impl std::fmt::Debug for DbHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbHandle").field("db", &self.url).finish()
    }
}

/// Lifecycle
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

    /// A new instance connecting to the project database at the given `url`.
    #[instrument(level = "debug", skip(url), err(Debug))]
    pub fn new_at_url(url: impl Into<String>) -> anyhow::Result<Self> {
        let url = url.into();
        let mut conn = rusqlite::Connection::open(&url)?;
        improve_concurrency(&conn)?;
        run_migrations(&mut conn)?;
        Ok(DbHandle { conn, url })
    }

    /// Return the path to the standard database file.
    pub fn db_file_path(db_dir: impl AsRef<Path>) -> PathBuf {
        db_dir.as_ref().join(FILE_NAME)
    }
}

fn run_migrations(conn: &mut rusqlite::Connection) -> anyhow::Result<()> {
    let policy = backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(std::time::Duration::from_millis(500)))
        .build();

    backoff::retry(policy, || {
        let count = migration::run(conn, migration::ours())?;
        if count > 0 {
            tracing::info!("Database updated with {count} migrations");
        }
        Ok::<_, backoff::Error<migration::Error>>(())
    })?;
    Ok(())
}
