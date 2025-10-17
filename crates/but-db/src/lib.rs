use std::path::{Path, PathBuf};

use anyhow::Result;
use diesel::{Connection, SqliteConnection, connection::SimpleConnection};

const FILE_NAME: &str = "but.sqlite";

mod hunk_assignments;
pub use hunk_assignments::HunkAssignment;
mod butler_actions;
pub use butler_actions::ButlerAction;
mod schema;
mod workflows;
pub use workflows::Workflow;
mod claude;
pub use claude::{ClaudeMessage, ClaudePermissionRequest, ClaudeSession};
mod file_write_locks;
pub use file_write_locks::FileWriteLock;
mod workspace_rules;
pub use workspace_rules::WorkspaceRule;
mod worktrees;
pub use worktrees::Worktree;
mod gerrit_metadata;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
pub use gerrit_metadata::GerritMeta;
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
    pub fn new_at_url(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        let mut conn = SqliteConnection::establish(&url)?;
        improve_concurrency(&mut conn)?;
        run_migrations(&mut conn)?;
        Ok(DbHandle { conn, url })
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

fn run_migrations(
    connection: &mut SqliteConnection,
) -> Result<Vec<diesel::migration::MigrationVersion<'_>>> {
    match connection.run_pending_migrations(MIGRATIONS) {
        Ok(migrations) => Ok(migrations),
        Err(e) => anyhow::bail!("Failed to run migrations: {}", e),
    }
}

/// polling primitives
pub mod poll;
