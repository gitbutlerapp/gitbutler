//! Database migration support.

use rusqlite::ErrorCode;
use std::time::Duration;

/// A migration with its creation timestamp for ordering.
#[derive(Copy, Clone, Debug)]
pub struct M<'a> {
    /// The SQL statement to execute for this migration.
    pub up: &'a str,
    /// The creation time in format like `20250529110746`, for sorting.
    pub up_created_at: u64,
}

impl<'a> M<'a> {
    /// Create a new migration with the given creation timestamp and SQL.
    pub const fn up(created_at_for_sorting: u64, up_sql: &'a str) -> Self {
        M {
            up: up_sql,
            up_created_at: created_at_for_sorting,
        }
    }
}

/// The time we wait at most if the database is locked or busy.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Run the given migrations on the connection.
pub fn run<'m>(
    conn: &mut rusqlite::Connection,
    migrations: impl IntoIterator<Item = M<'m>>,
) -> Result<usize, rusqlite::Error> {
    let trans = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)?;

    let migrations = {
        let mut v: Vec<_> = migrations.into_iter().collect();
        v.sort_by_key(|m| m.up_created_at);
        v
    };

    // Ensure the schema migrations table exists
    trans.execute_batch(SCHEMA_MIGRATION_TABLE)?;

    // Get existing versions
    let existing_versions = {
        let mut stmt = trans.prepare("SELECT version FROM __schema_migrations ORDER BY version")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    // Validate existing migrations match
    for (idx, existing_version) in existing_versions.iter().enumerate() {
        let existing_version: u64 = existing_version.parse().map_err(|_| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!(
                    "corrupted __schema_migrations table: invalid version '{}' at index {}",
                    existing_version, idx
                )),
            )
        })?;

        if idx >= migrations.len() {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!(
                    "database has {} migrations but code only has {} - are you running an older version?",
                    existing_versions.len(),
                    migrations.len()
                )),
            ));
        }

        let candidate_m = migrations[idx];
        if candidate_m.up_created_at != existing_version {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!(
                    "migration version mismatch at index {}: database has {}, code has {}",
                    idx, existing_version, candidate_m.up_created_at
                )),
            ));
        }
    }

    let mut count = 0;
    // Run only new migrations
    for migration in migrations.iter().skip(existing_versions.len()) {
        trans.execute_batch(migration.up).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!(
                    "failed to execute migration {}: {}",
                    migration.up_created_at, e
                )),
            )
        })?;

        let version = migration.up_created_at.to_string();
        trans.execute("INSERT INTO __schema_migrations (version) VALUES (?1)", [&version])?;

        count += 1;
    }

    trans.commit()?;
    Ok(count)
}

/// Improve database concurrency with WAL mode and appropriate settings.
pub fn improve_concurrency(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let query = r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA wal_autocheckpoint = 1000;
    "#;
    conn.execute_batch(query)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(())
}

const SCHEMA_MIGRATION_TABLE: &str = "CREATE TABLE IF NOT EXISTS __schema_migrations (
    version VARCHAR(50) PRIMARY KEY NOT NULL,
    run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
)";

/// Check if an error is transient (locked/busy).
pub fn is_transient_error(err: &rusqlite::Error) -> bool {
    err.sqlite_error_code()
        .is_some_and(|code| matches!(code, ErrorCode::DatabaseLocked | ErrorCode::DatabaseBusy))
}
