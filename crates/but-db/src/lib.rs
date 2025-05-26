use std::path::Path;

use anyhow::Result;
use diesel::{Connection, SqliteConnection};

const FILE_NAME: &str = "gb.sqlite";

pub mod hunk_assignments;
pub mod models;
mod schema;

fn connection_with_migrations(db_path: &Path) -> Result<SqliteConnection> {
    let db_file_path = db_path.join(FILE_NAME);
    let db_file_path = db_file_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in database file path"))?;
    let database_url = format!("file://{}", db_file_path);
    let mut conn = SqliteConnection::establish(&database_url)?;
    run_migrations(&mut conn)?;
    Ok(conn)
}

use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

fn run_migrations(
    connection: &mut SqliteConnection,
) -> Result<Vec<diesel::migration::MigrationVersion>> {
    match connection.run_pending_migrations(MIGRATIONS) {
        Ok(migrations) => Ok(migrations),
        Err(e) => anyhow::bail!("Failed to run migrations: {}", e),
    }
}

pub struct DbHandle {
    conn: SqliteConnection,
}

/// A handle to the database connection.
impl DbHandle {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = connection_with_migrations(db_path)?;
        Ok(DbHandle { conn })
    }
}
