use diesel::SqliteConnection;

const FILE_NAME: &str = "but.sqlite";

pub mod migration;

mod handle;
pub mod poll;
mod schema;

mod table;
#[rustfmt::skip]
pub use table::{
    hunk_assignments::HunkAssignment,
    butler_actions::ButlerAction,
    workflows::Workflow,
    claude::{ClaudeMessage, ClaudePermissionRequest, ClaudeSession},
    file_write_locks::FileWriteLock,
    workspace_rules::WorkspaceRule,
    gerrit_metadata::GerritMeta,
    forge_reviews::ForgeReview,
    ci_checks::CiCheck,
};

use diesel_migrations::{EmbeddedMigrations, embed_migrations};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

/// All migrations to run, in any order, a ordering is maintained by their date string.
pub const MIGRATIONS2: &[&[M<'static>]] = &[
    table::hunk_assignments::M,
    table::butler_actions::M,
    table::workflows::M,
    table::claude::M,
    table::file_write_locks::M,
    table::workspace_rules::M,
    table::gerrit_metadata::M,
    table::M_FULLY_REMOVED,
    table::forge_reviews::M,
    table::ci_checks::M,
];

/// A migration and all the necessary data associated with it.
///
/// Note that it's Diesel compatible.
#[derive(Copy, Clone, Debug)]
pub struct M<'a> {
    /// The SQL statement to execute for this migration.
    up: &'a str,
    /// The creation time of the `up` field, in a format like `20250529110746`, so it's suitable for sorting
    up_created_at: u64,
}

pub struct DbHandle {
    conn: SqliteConnection,
    /// The URL at which the connection was opened, mainly for debugging.
    url: String,
}
