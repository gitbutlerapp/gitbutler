use std::path::{Path, PathBuf};

use tracing::instrument;

use crate::{DbHandle, migration, migration::improve_concurrency};

const FILE_NAME: &str = "but.sqlite";

impl std::fmt::Debug for DbHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbHandle").field("db", &self.path).finish()
    }
}

/// Lifecycle
impl DbHandle {
    /// Create a new instance connecting to a file-based database contained in `db_dir`.
    /// It will be created or updated automatically.
    pub fn new_in_directory(db_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db_file_path = Self::db_file_path(db_dir);
        if let Some(parent_dir_to_create) = db_file_path.parent().filter(|dir| !dir.exists()) {
            std::fs::create_dir_all(parent_dir_to_create)?;
        }
        Self::new_at_path(db_file_path)
    }

    /// A new instance connecting to the project database at the given `path`.
    #[instrument(
        name = "DbHandle::new_at_path",
        level = "debug",
        skip(path),
        err(Debug)
    )]
    pub fn new_at_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let mut conn = rusqlite::Connection::open(&path)?;
        improve_concurrency(&conn)?;
        run_migrations(&mut conn)?;
        Ok(DbHandle { conn, path })
    }

    /// Return the path to the standard database file.
    pub fn db_file_path(db_dir: impl AsRef<Path>) -> PathBuf {
        db_dir.as_ref().join(FILE_NAME)
    }
}

fn run_migrations(conn: &mut rusqlite::Connection) -> anyhow::Result<()> {
    crate::backoff(|| {
        let count = migration::run(conn, migration::ours())?;
        if count > 0 {
            tracing::info!("Database updated with {count} migrations");
        }
        Ok::<_, migration::Error>(())
    })?;
    Ok(())
}
