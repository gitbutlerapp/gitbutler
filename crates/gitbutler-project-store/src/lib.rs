use anyhow::{anyhow, Context, Result};
use rusqlite::Connection;
use std::path::Path;

const DATABASE_NAME: &str = "project.sqlite";

/// ProjectStore provides a light wrapper around a sqlite database
struct ProjectStore {
    connection: Connection,
}

/// Database setup
///
/// Before touching any database related code, please read https://github.com/the-lean-crate/criner/discussions/5 first.
impl ProjectStore {
    /// Creates an instance of ProjectStore and runs any pending sqlite migrations
    /// gitbutler_project_directory should be the `.git/gitbutler` path of a given
    /// repository
    pub fn initialize(gitbutler_project_directory: &Path) -> Result<ProjectStore> {
        let database_path = gitbutler_project_directory.join(DATABASE_NAME);
        let database_path = database_path.to_str().ok_or(anyhow!(
            "Failed to get database {}",
            gitbutler_project_directory.display()
        ))?;

        let connection = Connection::open(database_path)?;

        ProjectStore::configure_connection(&connection)?;

        let project_store = ProjectStore { connection };

        project_store.run_migrations()?;

        Ok(project_store)
    }

    /// Configures a sqlite connection to behave sensibly in a concurrent environemnt.
    ///
    /// Busy handler and pargma's have been taken from https://github.com/the-lean-crate/criner/discussions/5
    /// and will help with concurrent reading and writing.
    ///
    /// This should be run before a project store is created and any other SQL is run.
    fn configure_connection(connection: &Connection) -> Result<()> {
        connection
            .busy_handler(Some(sleeper))
            .context("Failed to set connection's busy handler")?;

        connection.execute_batch("
            PRAGMA journal_mode = WAL;          -- better write-concurrency
            PRAGMA synchronous = NORMAL;        -- fsync only in critical moments
            PRAGMA wal_autocheckpoint = 1000;   -- write WAL changes back every 1000 pages, for an in average 1MB WAL file. May affect readers if number is increased
            PRAGMA wal_checkpoint(TRUNCATE);    -- free some space by truncating possibly massive WAL files from the last run.
        ").context("Failed to set PRAGMA's for connection")?;

        Ok(())
    }

    fn run_migrations(&self) -> Result<()> {
        Ok(())
    }
}

fn sleeper(attempts: i32) -> bool {
    println!("SQLITE_BUSY, retrying after 50ms (attempt {})", attempts);
    std::thread::sleep(std::time::Duration::from_millis(50));
    true
}
