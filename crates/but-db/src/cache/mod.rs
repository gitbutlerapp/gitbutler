use std::path::Path;

use rusqlite::ErrorCode;

/// Storage for cache tables, along with their types.
///
/// ### Usage
///
/// Caches work the same as [`crate::table`]s.
/// They are made more usable by providing a mutable instance through a shared reference
/// when obtaining it from the `but-ctx::Context`.
use crate::M;

mod handle;
mod table;

#[rustfmt::skip]
pub use table::{
    update::{CachedCheckResult, CheckUpdateStatus},
};

/// Open `url` with migrations applied. However, we don't retry as:
/// - if the database is locked, and we'd need to run migrations, we fall back to an in-memory DB just this time
/// - if we don't need to run migrations, all is good anyway, and we figure this out in read-only mode of the migration.
fn run_migrations<'m>(
    conn: &mut rusqlite::Connection,
    migrations: impl IntoIterator<Item = M<'m>> + Clone,
) -> Result<(), crate::migration::Error> {
    crate::migration::run(conn, migrations).map(|_| ())?;
    Ok(())
}

/// Like [`run_migrations`], but made so that it cannot fail **and** opens the database either
/// from `path`, removing broken ones on the fly, or from `:memory:` as final fallback,
/// returning `(conn, actual_url)`.
///
/// # Panics
///
/// If in-memory databases can't be opened **and** migrations from zero don't work.
/// Migrations are tested from zero, so that should be impossible.
fn open_with_migrations_infallible<'p, 'm>(
    path: &'p Path,
    migrations: impl IntoIterator<Item = M<'m>> + Clone,
) -> (rusqlite::Connection, &'p Path) {
    let mem_url = ":memory:".as_ref();
    let res = rusqlite::Connection::open(path).map(|c| (c, path));
    let (mut conn, mut path) = res
        .or_else(|path_err| {
            if path == mem_url {
                panic!("FATAL: Couldn't open in-memory URL: {path_err}")
            }
            tracing::warn!(
                "Failed to open cache database at '{path}' with {path_err}, will use memory DB instead",
                path = path.display()
            );
            rusqlite::Connection::open(mem_url)
                .map(|c| (c, mem_url))
                .map_err(|memory_err| {
                    anyhow::Error::from(memory_err).context(path_err).context(format!(
                        "Couldn't open database either from {path} or in memory",
                        path = path.display()
                    ))
                })
        })
        .expect("FATAL: didn't expect to not be able to open an in-memory database at least");

    if let Err(err) = run_migrations(&mut conn, migrations.clone()) {
        assert_ne!(
            path,
            mem_url,
            "BUG: migrations from zero failed in memory DB after permanently failing to open {path}: {err}",
            path = path.display()
        );
        drop(conn);
        let (backoff::Error::Transient { err, .. } | backoff::Error::Permanent(err)) = err;
        if err.sqlite_error_code().is_some_and(is_invalid_database) {
            if let Err(err) = std::fs::remove_file(path) {
                tracing::warn!(
                    ?err,
                    "Failed to delete cache database at {path}, using in-memory one instead",
                    path = path.display()
                );
            } else {
                match rusqlite::Connection::open(path) {
                    Ok(mut conn) => match run_migrations(&mut conn, migrations.clone()) {
                        Ok(_) => {
                            crate::migration::improve_concurrency(&conn).ok();
                            return (conn, path);
                        }
                        Err(err) => {
                            tracing::warn!(
                                ?err,
                                "Failed to run migration on newly opened database at '{path}' - retrying with in-memory one",
                                path = path.display(),
                            )
                        }
                    },
                    Err(err) => tracing::warn!(
                        ?err,
                        "Path at '{path}' not writable, falling back to in-memory database",
                        path = path.display(),
                    ),
                }
            }
        }
        path = mem_url;
        conn = rusqlite::Connection::open(path).expect("FATAL: failed to open memory database run migrations on");
        run_migrations(&mut conn, migrations).expect("BUG: migrations on in-memory database should never fail");
    }

    if path == mem_url {
        tracing::error!(
            "Caching will work, but caches won't persist, leading to sub-par performance, as {path} could not be written to.",
            path = path.display()
        );
    }

    if let Err(err) = crate::migration::improve_concurrency(&conn) {
        tracing::warn!(?err, "Failed to improve concurrency - continuing without");
    }
    (conn, path)
}

fn is_invalid_database(code: ErrorCode) -> bool {
    matches!(code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
}

#[cfg(test)]
mod tests;
