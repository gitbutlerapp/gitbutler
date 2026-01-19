/// Storage for cache tables, along with their types.
///
/// ### Usage
///
/// Cache work the same as [`crate::table`]s. They are made more usable by providing a mutable instance through a shared reference
/// when obtaining it from the `but-ctx::Context`.
use crate::M;

mod handle;
mod table;

#[rustfmt::skip]
pub use table::{
    update::{CachedCheckResult, CheckUpdateStatus},
};

/// Open `url` with migrations applied. These can fail.
fn run_migrations<'m>(
    conn: &mut rusqlite::Connection,
    migrations: impl IntoIterator<Item = M<'m>> + Clone,
) -> Result<(), crate::migration::Error> {
    let policy = backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(std::time::Duration::from_secs(1).into())
        .build();
    let migrations = Vec::from_iter(migrations);
    backoff::retry(policy, || {
        crate::migration::run(conn, migrations.clone()).map(|_| ())
    })?;
    Ok(())
}

/// Like [`run_migrations`], but made so that it cannot fail **and** opens the database either
/// from `url`, removing broken ones on the fly, or from `:memory:` as final fallback,
/// returning `(conn, actual_url)`.
///
/// # Panics
///
/// If in-memory databases can't be opened **and** migrations from zero don't work.
/// Migrations are tested from zero, so that should be impossible.
fn open_with_migrations_infallible<'m>(
    url: &str,
    migrations: impl IntoIterator<Item = M<'m>> + Clone,
) -> (rusqlite::Connection, &str) {
    let mem_url = ":memory:";
    let res = if url == mem_url {
        rusqlite::Connection::open(url)
    } else {
        rusqlite::Connection::open_with_flags(url, rusqlite::OpenFlags::SQLITE_OPEN_EXRESCODE)
    }
    .map(|c| (c, url));

    let (mut conn, mut url) = res
        .or_else(|url_err| {
            if url == mem_url {
                panic!("FATAL: Couldn't open in-memory URL: {url_err}")
            }
            tracing::warn!("Failed to open cache database at '{url}' - will try to recreate it by removing the broken one");
            if let Err(err) = std::fs::remove_file(url) {
                tracing::warn!(
                    ?err,
                    "Failed to delete cache database at {url}, using in-memory one instead"
                );
            } else {
                match rusqlite::Connection::open(url).map(|c| (c, url)) {
                    Ok(res) => return Ok(res),
                    Err(err) => tracing::warn!(
                        ?err,
                        "Url at '{url}' not writable, falling back to in-memory database"
                    ),
                }
            }
            rusqlite::Connection::open(mem_url)
                .map(|c| (c, mem_url))
                .map_err(|memory_err| {
                    anyhow::Error::from(memory_err)
                        .context(url_err)
                        .context(format!(
                            "Couldn't open database either from {mem_url} or in memory"
                        ))
                })
        })
        .expect("FATAL: didn't expect to not be able to open an in-memory database at least");

    if let Err(err) = crate::migration::improve_concurrency(&conn) {
        tracing::warn!(?err, "Failed to improve concurrency - continuing without");
    }
    if let Err(err) = run_migrations(&mut conn, migrations.clone()) {
        assert_ne!(
            url, mem_url,
            "BUG: migrations from zero failed in memory DB after permanently failing to open {url}: {err}"
        );
        drop(conn);
        url = mem_url;
        conn = rusqlite::Connection::open(url)
            .expect("FATAL: failed to open memory database at {url} to run migrations on");
        run_migrations(&mut conn, migrations)
            .expect("BUG: migrations on in-memory database should never fail");
    }

    if url == mem_url {
        tracing::error!(
            "Caching will work, but caches won't persist, leading to sub-par performance, as {url} could not be written to"
        );
    }

    (conn, url)
}

#[cfg(test)]
mod tests;
