use anyhow::{anyhow, Result};
use std::path::Path;

use diesel::{Connection, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

const DATABASE_NAME: &str = "project.sqlite";
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// ProjectStore provides a light wrapper around a sqlite database
struct ProjectStore {
    connection: SqliteConnection,
}

/// Database setup
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
        let mut connection = SqliteConnection::establish(database_path)?;

        // Run any pending migrations
        connection
            .run_pending_migrations(MIGRATIONS)
            .map_err(|err| anyhow!("{}", err))?;

        Ok(ProjectStore { connection })
    }
}
