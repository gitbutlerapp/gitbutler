use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Context as _;
use but_utils::OnDemand;
use rusqlite::{OpenFlags, backup::StepResult};
use tracing::instrument;

use crate::{CacheHandle, DbHandle, migration, migration::improve_concurrency};

const FILE_NAME: &str = "but.sqlite";

impl std::fmt::Debug for DbHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbHandle").field("db", &self.path).finish()
    }
}

/// Lifecycle
impl DbHandle {
    /// Create a new instance connecting to a file-based database contained in `db_dir`.
    /// It will be created or updated automatically.
    pub fn new_in_directory(db_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db_file_path = Self::db_file_path(db_dir);
        if let Some(parent_dir_to_create) = db_file_path.parent().filter(|dir| !dir.exists()) {
            std::fs::create_dir_all(parent_dir_to_create)?;
        }
        Self::new_at_path(db_file_path)
    }

    /// Open the project database for read-only access if it already exists.
    ///
    /// Unlike [`Self::new_in_directory()`], this never creates parent directories,
    /// creates the database file, or runs migrations. The returned handle owns a
    /// writable in-memory clone whose changes are discarded when it is dropped.
    pub fn open_existing_read_only_in_directory(
        db_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Option<Self>> {
        let db_file_path = Self::db_file_path(db_dir);
        if !db_file_path.try_exists().with_context(|| {
            format!(
                "Check whether project database exists at {}",
                db_file_path.display()
            )
        })? {
            return Ok(None);
        }
        open_existing_read_only_at_path_optional(db_file_path)
    }

    /// Open an existing project database as a writable in-memory clone.
    ///
    /// The source is opened read-only and is never migrated. Changes made through
    /// the returned handle are process-local and discarded when it is dropped.
    #[instrument(
        name = "DbHandle::open_existing_read_only_at_path",
        level = "trace",
        skip(path),
        err(Debug)
    )]
    pub fn open_existing_read_only_at_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        open_existing_read_only_at_path_optional(path.clone())?
            .ok_or_else(|| anyhow::anyhow!("Project database does not exist at {}", path.display()))
    }

    /// A new instance connecting to the project database at the given `path`.
    #[instrument(
        name = "DbHandle::new_at_path",
        level = "trace",
        skip(path),
        err(Debug)
    )]
    pub fn new_at_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let mut conn = rusqlite::Connection::open(&path)?;
        improve_concurrency(&conn)?;
        run_migrations(&mut conn)?;
        let cache = cache_for_db_path(&path);
        Ok(DbHandle { conn, path, cache })
    }

    /// Return the path to the standard database file.
    pub fn db_file_path(db_dir: impl AsRef<Path>) -> PathBuf {
        db_dir.as_ref().join(FILE_NAME)
    }
}

fn open_existing_read_only_at_path_optional(path: PathBuf) -> anyhow::Result<Option<DbHandle>> {
    if !path.try_exists().with_context(|| {
        format!(
            "Check whether project database exists at {}",
            path.display()
        )
    })? {
        return Ok(None);
    }

    let conn = clone_database_into_memory(&path)?;

    let cache = OnDemand::new(|| Ok(CacheHandle::new_at_path(":memory:")));
    Ok(Some(DbHandle { conn, path, cache }))
}

fn clone_database_into_memory(path: &Path) -> anyhow::Result<rusqlite::Connection> {
    clone_database_into_memory_with(path, || Ok(()))
}

fn clone_database_into_memory_with(
    path: &Path,
    mut after_first_snapshot: impl FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<rusqlite::Connection> {
    let started = Instant::now();
    loop {
        let first = SourceSnapshot::capture(path)?;
        after_first_snapshot()?;
        let second = SourceSnapshot::capture(path)?;
        if first != second {
            retry_changed_snapshot(started)?;
            continue;
        }

        let clone = clone_snapshot_into_memory(&second)?;
        if second == SourceSnapshot::capture(path)? {
            return Ok(clone);
        }
        retry_changed_snapshot(started)?;
    }
}

fn retry_changed_snapshot(started: Instant) -> anyhow::Result<()> {
    if started.elapsed() >= migration::BUSY_TIMEOUT {
        anyhow::bail!("Project database kept changing while copying it into memory");
    }
    std::thread::sleep(Duration::from_millis(10));
    Ok(())
}

fn clone_snapshot_into_memory(snapshot: &SourceSnapshot) -> anyhow::Result<rusqlite::Connection> {
    let temp_dir = tempfile::tempdir().context("Create private project database snapshot")?;
    let snapshot_path = temp_dir.path().join(FILE_NAME);
    std::fs::write(&snapshot_path, &snapshot.database)
        .context("Materialize private project database snapshot")?;
    if let Some(wal) = snapshot.wal.as_ref().filter(|wal| !wal.is_empty()) {
        std::fs::write(sidecar_path(&snapshot_path, "-wal"), wal)
            .context("Materialize private project database WAL snapshot")?;
    }
    clone_materialized_database_into_memory(&snapshot_path)
}

fn clone_materialized_database_into_memory(path: &Path) -> anyhow::Result<rusqlite::Connection> {
    let source = rusqlite::Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| {
            format!(
                "Open private project database snapshot at {}",
                path.display()
            )
        })?;
    let page_size: i64 = source
        .pragma_query_value(None, "page_size", |row| row.get(0))
        .with_context(|| format!("Read project database page size at {}", path.display()))?;

    let mut conn = rusqlite::Connection::open_in_memory()
        .context("Create in-memory project database clone")?;
    conn.pragma_update(None, "page_size", page_size)
        .context("Match in-memory project database page size")?;

    {
        let backup = rusqlite::backup::Backup::new(&source, &mut conn)
            .context("Start in-memory project database backup")?;
        let mut contention_started = None;
        loop {
            match backup
                .step(128)
                .context("Copy project database into memory")?
            {
                StepResult::Done => break,
                StepResult::More => contention_started = None,
                StepResult::Busy => {
                    let started = contention_started.get_or_insert_with(Instant::now);
                    if started.elapsed() >= migration::BUSY_TIMEOUT {
                        anyhow::bail!("Project database stayed busy while copying it into memory");
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                StepResult::Locked => {
                    let started = contention_started.get_or_insert_with(Instant::now);
                    if started.elapsed() >= migration::BUSY_TIMEOUT {
                        anyhow::bail!(
                            "Project database stayed locked while copying it into memory"
                        );
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => anyhow::bail!("In-memory project database backup returned an unknown result"),
            }
        }
    }
    improve_concurrency(&conn).context("Configure in-memory project database clone")?;

    Ok(conn)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceSnapshot {
    database: Vec<u8>,
    wal: Option<Vec<u8>>,
}

impl SourceSnapshot {
    fn capture(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            database: std::fs::read(path)
                .with_context(|| format!("Read project database at {}", path.display()))?,
            wal: read_optional(&sidecar_path(path, "-wal"), "WAL")?,
        })
    }
}

fn read_optional(path: &Path, name: &str) -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => {
            Err(err).with_context(|| format!("Read project database {name} at {}", path.display()))
        }
    }
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    value.into()
}

fn cache_for_db_path(path: &Path) -> OnDemand<CacheHandle> {
    if path == Path::new(":memory:") {
        return OnDemand::new(|| Ok(CacheHandle::new_at_path(":memory:")));
    }
    let cache_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    OnDemand::new(move || Ok(CacheHandle::new_in_directory(cache_dir.clone())))
}

fn run_migrations(conn: &mut rusqlite::Connection) -> anyhow::Result<()> {
    crate::backoff(|| {
        let count = migration::run(conn, migration::ours())?;
        if count > 0 {
            tracing::info!("Database updated with {count} migrations");
        }
        Ok::<_, migration::Error>(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_clone_enables_foreign_keys() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        DbHandle::new_in_directory(tmp.path())?;

        let clone = DbHandle::open_existing_read_only_in_directory(tmp.path())?
            .expect("source database exists");
        let enabled: bool = clone
            .conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))?;

        assert!(
            enabled,
            "writable clones must preserve foreign-key enforcement"
        );
        Ok(())
    }

    #[test]
    fn checkpointed_snapshot_retries_concurrent_checkpoint() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        DbHandle::new_in_directory(tmp.path())?;
        let path = DbHandle::db_file_path(tmp.path());
        let mut inject_checkpoint = true;

        let conn = clone_database_into_memory_with(&path, || {
            if inject_checkpoint {
                inject_checkpoint = false;
                let mut writer = DbHandle::new_in_directory(tmp.path())?;
                writer
                    .branch_order_mut()?
                    .set_order(&["refs/heads/A".to_owned()])?;
            }
            Ok(())
        })?;
        let clone = DbHandle {
            conn,
            path,
            cache: OnDemand::new(|| Ok(CacheHandle::new_at_path(":memory:"))),
        };

        assert_eq!(
            clone.branch_order().order_for_reference("refs/heads/A")?,
            Some(vec!["refs/heads/A".to_owned()]),
            "the retry must deserialize the stable post-checkpoint snapshot"
        );
        Ok(())
    }

    #[test]
    fn source_snapshot_ignores_shm_and_tracks_wal_content() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("but.sqlite");
        std::fs::write(&path, b"database")?;
        let wal_path = sidecar_path(&path, "-wal");
        let shm_path = sidecar_path(&path, "-shm");
        std::fs::write(&wal_path, b"wal before")?;
        std::fs::write(&shm_path, b"before")?;
        let before = SourceSnapshot::capture(&path)?;

        std::fs::write(&shm_path, b"after state")?;
        assert_eq!(
            before,
            SourceSnapshot::capture(&path)?,
            "SHM is derived coordination state and must not be copied"
        );

        std::fs::write(&wal_path, b"wal after")?;
        assert_ne!(
            before,
            SourceSnapshot::capture(&path)?,
            "snapshot guards must compare durable WAL content"
        );
        Ok(())
    }
}
