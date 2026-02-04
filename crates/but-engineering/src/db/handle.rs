//! Database connection handling.

use std::path::{Path, PathBuf};

use crate::db::migration::{self, M};
use crate::db::table;

const FILE_NAME: &str = "but-engineering.db";

/// Handle to the database connection.
pub struct DbHandle {
    /// The opened database connection.
    pub(crate) conn: rusqlite::Connection,
    /// The path to the database file.
    path: PathBuf,
}

impl std::fmt::Debug for DbHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbHandle").field("db", &self.path).finish()
    }
}

impl DbHandle {
    /// Create a new database handle at the standard location within the git directory.
    ///
    /// The database is created at `<git_dir>/gitbutler/but-engineering.db`.
    pub fn new_in_git_dir(git_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db_dir = git_dir.as_ref().join("gitbutler");
        if !db_dir.exists() {
            std::fs::create_dir_all(&db_dir)?;
        }
        let db_path = db_dir.join(FILE_NAME);
        Self::new_at_path(db_path)
    }

    /// Create a new database handle at the specified path.
    pub fn new_at_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let mut conn = rusqlite::Connection::open(&path)?;
        migration::improve_concurrency(&conn)?;
        run_migrations(&mut conn)?;
        Ok(DbHandle { conn, path })
    }

    /// Get the path to the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get a reference to the underlying connection.
    pub fn conn(&self) -> &rusqlite::Connection {
        &self.conn
    }
}

fn run_migrations(conn: &mut rusqlite::Connection) -> anyhow::Result<()> {
    migration::run(conn, all_migrations())?;
    Ok(())
}

fn all_migrations() -> impl Iterator<Item = M<'static>> {
    table::agents::M
        .iter()
        .chain(table::messages::M.iter())
        .chain(table::claims::M.iter())
        .chain(table::sessions::M.iter())
        .copied()
}
