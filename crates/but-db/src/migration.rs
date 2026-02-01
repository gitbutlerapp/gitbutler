use rusqlite::ErrorCode;
use tracing::instrument;

use crate::M;

/// The error produced when running migrations.
pub type Error = backoff::Error<rusqlite::Error>;

/// The time we wait at most if the database is locked or busy before giving up acquiring a lock.
pub(crate) const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Return a sequence of our own, well known migrations.
///
/// Note that these will be ordered by [`run()`].
pub fn ours() -> impl Iterator<Item = M<'static>> {
    crate::MIGRATIONS.iter().flat_map(|per_table| per_table.iter()).copied()
}

/// Run the given `migrations` on `conn`, returning the amount of migrations that ran.
///
/// Note that some errors can be retried, but for now there is no recovery beyond hoping
/// for an intermediate failure related to databases.
///
/// Currently, either all migrations succeed, or all fail.
#[instrument(name = "run_migration", level = "trace", skip(conn, migrations), err(Debug))]
pub fn run<'m>(conn: &mut rusqlite::Connection, migrations: impl IntoIterator<Item = M<'m>>) -> Result<usize, Error> {
    let trans = conn
        // Use deferred to allow ourselves to read first without running into locks.
        // That read can determine that nothing needs to be done, saving a lot of time.
        .transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)
        .map_err(transient_if_locked)?;
    let migrations = {
        let mut v: Vec<_> = migrations.into_iter().collect();
        v.sort_by_key(|m| m.up_created_at);
        v
    };
    // Bail early if our read detects that nothing is to be done
    let maybe_num_applied_versions = num_applied_versions(&trans, &migrations).ok();
    if let Some(count) = maybe_num_applied_versions
        && count == migrations.len()
    {
        return Ok(0);
    }

    let num_applied_consecutive_versions = match maybe_num_applied_versions {
        Some(count) => count,
        None => {
            // We couldn't read the table, be sure it exists and refresh the count just to be sure.
            trans
                .execute_batch(DIESEL_SCHEMA_MIGRATION_TABLE)
                .map_err(transient_if_locked)?;
            num_applied_versions(&trans, &migrations)?
        }
    };

    let mut count = 0;
    // Run only new migrations (after all existing ones)
    for migration in migrations.iter().skip(num_applied_consecutive_versions) {
        trans.execute_batch(migration.up).map_err(transient_if_locked)?;

        let version = migration.up_created_at.to_string();
        trans
            .execute(
                "INSERT INTO __diesel_schema_migrations (version) VALUES (?1)",
                [&version],
            )
            .map_err(transient_if_locked)?;

        count += 1;
    }

    trans.commit().map_err(transient_if_locked)?;
    Ok(count)
}

fn num_applied_versions(conn: &rusqlite::Connection, migrations: &[M]) -> Result<usize, Error> {
    let existing_versions = {
        let mut stmt = conn
            .prepare("SELECT version FROM __diesel_schema_migrations ORDER BY version")
            .map_err(transient_if_locked)?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(transient_if_locked)?;
        rows.into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(transient_if_locked)?
    };

    // Validate that existing migrations match the provided migrations list
    for (idx, existing_version) in existing_versions.iter().enumerate() {
        let existing_version: u64 = existing_version
            .parse()
            .map_err(|_| backoff::Error::permanent(rusqlite::Error::InvalidQuery))?;

        let err: Option<String> = if idx >= migrations.len() {
            format!(
                "Cannot reduce migration count: database has {num_existing} migrations but only {actual} provided",
                actual = migrations.len(),
                num_existing = existing_versions.len()
            )
            .into()
        } else {
            let candidate_m = migrations[idx];
            (candidate_m.up_created_at != existing_version).then(|| {
                format!(
                    "Migration {idx} should be of version {expected}, but got {actual}",
                    expected = existing_version,
                    actual = candidate_m.up_created_at
                )
            })
        };
        if let Some(err) = err {
            return Err(backoff::Error::permanent(rusqlite::Error::ToSqlConversionFailure(
                err.into(),
            )));
        }
    }

    Ok(existing_versions.len())
}

fn transient_if_locked(err: rusqlite::Error) -> Error {
    if err
        .sqlite_error_code()
        .is_some_and(|code| matches!(code, ErrorCode::DatabaseLocked | ErrorCode::DatabaseBusy))
    {
        backoff::Error::transient(err)
    } else {
        backoff::Error::permanent(err)
    }
}

impl<'a> M<'a> {
    /// Create a new migration with `created_at_for_sorting` in a format like `20250529110746`, and the `up_sql`
    /// which is Sqlite compatible SQL to create or update tables.
    pub const fn up(created_at_for_sorting: u64, up_sql: &'a str) -> Self {
        M {
            up: up_sql,
            up_created_at: created_at_for_sorting,
        }
    }
}

const DIESEL_SCHEMA_MIGRATION_TABLE: &str = "CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
       version VARCHAR(50) PRIMARY KEY NOT NULL,
       run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
)";

/// Improve parallelism and make it non-fatal.
/// Blanket setting from https://github.com/the-lean-crate/criner/discussions/5, maybe needs tuning.
/// Also, it's a known issue, maybe order matters?
/// https://github.com/diesel-rs/diesel/issues/2365#issuecomment-2899347817
/// TODO: the busy_timeout doesn't seem to be effective.
pub(crate) fn improve_concurrency(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let query = r#"
        PRAGMA journal_mode = WAL;               -- better write-concurrency,
        PRAGMA synchronous = NORMAL;             -- fsync only in critical moments,
        PRAGMA wal_autocheckpoint = 1000;        -- write WAL changes back every 1000 pages, for an average 1MB WAL file.,
        PRAGMA wal_checkpoint(TRUNCATE);         -- free some space by truncating possibly massive WAL files from the last run.,
        "#;
    conn.execute_batch(query)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(())
}
