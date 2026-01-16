//! # vORM - a vibe-code friendly ORM
//!
//! There are a couple of layers that work together to make this possible.
//!
//! ## `rusqlite` - sqlite for Rust
//!
//! It leverages the borrow-checker to respect mutability rules, while also allowing to bypass them if one wants to,
//! i.e. using `&Connection` to execute SQL.
//! And thanks to Rusts mutability rules and transactions, generally everything is possible and safe.
//!
//! ## Migrations - simple and safe
//!
//! Migrations are in-module and colocated with the ORM abstraction. That way the LLM can imagine the
//! final shape of the data, along with the shape of the corresponding Rust structure, which helps with
//! precision and type-safety.
//!
//! ## ORM Types - for `Connection` and `Transaction`
//!
//! All ORM types are split into read-only and mutating versions, and they are thin wrappers around a
//! [`Connection`](rusqlite::Connection) or a [`SavePoint`](rusqlite::Savepoint).
//! A savepoint is only used when changes need the transaction-like ability to be committed all at once.
//!
//! Read-only methods are implemented on read-only ORM types, and mutating methods
//! of any complexity are implemented on mutating types. Tests exists for each method just for basic
//! insurance they actually work.
//!
//! Mutating types can always turn themselves into the read-only counterparts, but not vice versa.
use diesel::SqliteConnection;

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

/// The migrations to run, in any order, as ordering is maintained by their date number.
pub const MIGRATIONS: &[&[M<'static>]] = &[
    table::M_FULLY_REMOVED,
    table::hunk_assignments::M,
    table::butler_actions::M,
    table::workflows::M,
    table::claude::M,
    table::file_write_locks::M,
    table::workspace_rules::M,
    table::gerrit_metadata::M,
    table::forge_reviews::M,
    table::ci_checks::M,
];

/// A migration and all the necessary data associated with it to perform it once.
///
/// Note that it's `diesel_migrations` compatible as it uses its database and schema
/// for historical reasons.
#[derive(Copy, Clone, Debug)]
pub struct M<'a> {
    /// The SQL statement to execute for this migration.
    up: &'a str,
    /// The creation time of the `up` field, in a format like `20250529110746`, so it's suitable for sorting
    up_created_at: u64,
}

/// An abstraction over an open database connection, and for access to the ORM layer and [transactions](DbHandle::transaction()).
///
/// The underlying sqlite database is set up to use Rusts borrow-checker,
/// so a mutable borrow is required to start transactions or to make changes to any data.
pub struct DbHandle {
    #[expect(dead_code)]
    diesel: SqliteConnection,
    conn: rusqlite::Connection,
    /// The URL at which the connection was opened, mainly for debugging.
    url: String,
}

/// A wrapper for a [`rusqlite::Transaction`] to allow ORM handles to be created more easily,
/// and make sure multiple dependent calls to the ORM can be consistent.
pub struct Transaction<'conn>(pub(crate) rusqlite::Transaction<'conn>);
