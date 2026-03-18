//! # An agent-friendly ORM
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
//! Each migration also passes a [`SchemaVersion`] into [`M::up`]. That version is about
//! forward-compatibility, not ordering. Keep the current version when older binaries can
//! still open the database safely after the migration, for example when the change is
//! additive or only affects data that older code ignores. Add the next schema-version
//! variant only when an older binary must reject the migrated database, for example after
//! dropping or repurposing persisted data that older code still depends on. Doing so
//! is the exception, and backward compatible changes are what we aim for.
//!
//! ## ORM Types - for `Connection` and `Transaction`
//!
//! All ORM types are split into read-only and mutating versions, and they are thin wrappers around a
//! [`Connection`](rusqlite::Connection) or a [`SavePoint`](rusqlite::Savepoint).
//! A *savepoint* is only used when changes need the transaction-like ability to be committed all at once.
//!
//! Read-only methods are implemented on read-only ORM types, and mutating methods
//! of any complexity are implemented on mutating types. Tests exists for each method just for basic
//! insurance they actually work.
//!
//! Mutating types can always turn themselves into the read-only counterparts, but not vice versa.
//!
//! # How to make changes
//!
//! Just ask your favorite LLM to make the changes for you, and they usually figure out how to do it
//! based on the existing tables.
//!
//! All of these have been proven to work perfectly, including migrations and tests.
//!
//! #### Add a new table
//!
//! ```text
//! Use @crates/but-db/src/lib.rs as starting point to add a new table with all information
//! one would need to list, insert, update and delete TODO list items. Do write tests.
//! ```
//!
//! #### Add a new field
//!
//! ```text
//! Add a new optional string named 'note' to ClaudePermissionRequest in @crates/but-db/tests/db/table/claude.rs
//! ```
#![expect(clippy::inconsistent_digit_grouping)]
#![deny(missing_docs)]
use rusqlite::ErrorCode;
use std::path::PathBuf;

#[cfg(feature = "poll")]
/// Polling helpers to watch for database-backed state changes.
pub mod poll;

mod handle;
mod table;
mod transaction;

/// Cache database helpers and typed accessors.
pub mod cache;
/// Migration helpers for applying and configuring database schema updates.
pub mod migration;

#[rustfmt::skip]
pub use table::{
    hunk_assignments::{HunkAssignmentsHandleMut, HunkAssignmentsHandle, HunkAssignment},
    butler_actions::ButlerAction,
    workflows::Workflow,
    claude::{ClaudeMessage, ClaudePermissionRequest, ClaudeSession},
    file_write_locks::FileWriteLock,
    workspace_rules::WorkspaceRule,
    gerrit_metadata::GerritMeta,
    forge_reviews::ForgeReview,
    ci_checks::CiCheck,
    virtual_branches::{VbBranchTarget, VbStack, VbStackHead, VbState, VirtualBranchesSnapshot, VirtualBranchesHandle, VirtualBranchesHandleMut},
};

/// The *retryable* error type used by [`backoff()`] for SQLite operations.
pub type Error = ::backoff::Error<rusqlite::Error>;

/// Retry a short SQLite operation that was intentionally configured to fail fast on lock contention.
///
/// This is meant for operations that should prefer responsiveness over waiting for a database lock.
/// You *may* pair it with [`Transaction::set_nonblocking()`] or one of the `*_nonblocking` transaction acquisition
/// functions so each attempt fails immediately with `SQLITE_BUSY` or `SQLITE_LOCKED` instead of spending
/// the whole retry budget in one blocking call.
/// This function *must* be used if a [`Transaction`] is used lock in a read, followed by a write, as the
/// deferred write lock acquisition may fail immediately if another write happened while you were reading.
///
/// Consider using [`map_err()`] to decide which database errors can be retried.
///
/// Only lock-contention errors are retried. Everything else is treated as permanent and returned immediately.
pub fn backoff<T, E>(
    operation: impl FnMut() -> Result<T, ::backoff::Error<E>>,
) -> Result<T, ::backoff::Error<E>> {
    let policy = ::backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(std::time::Duration::from_millis(500)))
        .build();
    ::backoff::retry(policy, operation)
}

/// Classify SQLite lock-contention failures as transient so [`backoff()`] can retry them.
///
/// This is the adapter to use with `Result::map_err()` for database operations that participate in
/// fail-fast retry loops.
pub fn map_err(err: rusqlite::Error) -> ::backoff::Error<rusqlite::Error> {
    if err
        .sqlite_error_code()
        .is_some_and(|code| matches!(code, ErrorCode::DatabaseLocked | ErrorCode::DatabaseBusy))
    {
        ::backoff::Error::transient(err)
    } else {
        ::backoff::Error::permanent(err)
    }
}

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
    table::virtual_branches::M,
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
    /// The forward-compatibility schema version after this migration has been applied.
    schema_version: SchemaVersion,
}

/// Documents the forward-compatibility boundary associated with a migration.
///
/// The numeric migration id remains the source of truth for ordering. This enum exists so
/// each migration must state whether it crosses into a new forward-incompatible database
/// shape or remains readable by older client binaries.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SchemaVersion {
    /// The current forward-compatible schema line.
    ///
    /// Keep using `Zero` for migrations that older binaries can still tolerate after the
    /// migration runs, such as adding tables or columns that they don't require.
    ///
    /// Switch to `One` only once a migration makes the database unsafe for binaries that only
    /// understand `Zero`, such as removing or reinterpreting persisted data they still use.
    Zero = 0,
    /// The first forward-incompatible schema line.
    ///
    /// Use `One` once a migration requires older `Zero`-only binaries to reject the
    /// database, and keep using it until the next forward-incompatible boundary is introduced.
    /// Document here WHY the schema is breaking application forward compatibility.
    One = 1,
}

/// A structure to receive an application-wide cache.
pub struct AppCacheHandle {
    /// The open connection to the cache.
    conn: rusqlite::Connection,
    /// The path to the application cache.
    path: PathBuf,
}

/// A structure to receive a project-local cache.
pub struct CacheHandle {
    /// The open connection to the cache.
    conn: rusqlite::Connection,
    /// The path to the project-local cache.
    path: PathBuf,
}

/// An abstraction over an open database connection, and for access to the ORM layer and [transactions](DbHandle::transaction()).
///
/// The underlying sqlite database is set up to use Rusts borrow-checker,
/// so a mutable borrow is required to start transactions or to make changes to any data.
pub struct DbHandle {
    /// The opened db connection with migrations applied.
    conn: rusqlite::Connection,
    /// The path at which the connection was opened, mainly for debugging.
    path: PathBuf,
}

/// A wrapper for a [`rusqlite::Transaction`] to allow ORM handles to be created more easily,
/// and make sure multiple dependent calls to the ORM can be consistent.
pub struct Transaction<'conn> {
    /// The actual transaction as holder of the database connection.
    /// It's always set.
    inner: Option<rusqlite::Transaction<'conn>>,
    /// If `true`, on drop we will reset the busy timeout to the default value, as previously the connection
    /// was changed to non-blocking.
    reset_to_blocking_on_drop: bool,
}
