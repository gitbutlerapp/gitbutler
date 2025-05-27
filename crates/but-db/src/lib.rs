use std::path::Path;

use anyhow::Result;
use diesel::{Connection, SqliteConnection};

const FILE_NAME: &str = "gb.sqlite";

pub mod hunk_assignments;
pub mod models;
mod schema;

use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub struct DbHandle {
    conn: SqliteConnection,
    /// The URL at which the connection was opened, mainly for debugging.
    url: String,
}

impl std::fmt::Debug for DbHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbHandle").field("db", &self.url).finish()
    }
}

/// A handle to the database connection.
impl DbHandle {
    /// Create a new instance connecting to a file-based database contained in `db_dir`.
    /// It will be created or updated automatically.
    pub fn new_in_directory(db_dir: impl AsRef<Path>) -> Result<Self> {
        let mut db_dir = db_dir.as_ref().to_owned();
        let cwd = std::env::current_dir()?;
        if db_dir.is_relative() {
            db_dir = cwd.join(db_dir);
        }
        let db_file_path = db_dir.join(FILE_NAME);
        if let Some(parent_dir_to_create) = db_file_path.parent().filter(|dir| !dir.exists()) {
            std::fs::create_dir_all(parent_dir_to_create)?;
        }
        let db_file_path = db_file_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in database file path"))?;
        Self::new_at_url(format!("file://{}", db_file_path))
    }

    /// A new instance connecting to the database at the given `url`.
    pub fn new_at_url(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        let mut conn = SqliteConnection::establish(&url)?;
        run_migrations(&mut conn)?;
        Ok(DbHandle { conn, url })
    }
}

fn run_migrations(
    connection: &mut SqliteConnection,
) -> Result<Vec<diesel::migration::MigrationVersion>> {
    match connection.run_pending_migrations(MIGRATIONS) {
        Ok(migrations) => Ok(migrations),
        Err(e) => anyhow::bail!("Failed to run migrations: {}", e),
    }
}
