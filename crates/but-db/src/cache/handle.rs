use std::path::{Path, PathBuf};

use tracing::instrument;

use crate::{AppCacheHandle, CacheHandle};

/// Lifecycle
impl AppCacheHandle {
    /// Infallible constructor that opens from `dir` or from memory if that is `None`,
    /// with an infallible constructor that falls back to an in-memory database.
    pub fn new_in_directory(dir: Option<impl AsRef<Path>>) -> Self {
        let db_path = dir.map_or(":memory:".into(), |d| d.as_ref().join("app-cache.sqlite"));
        Self::new_at_path(db_path)
    }

    /// Create a new instance at `path`.
    #[instrument(name = "AppCacheHandle::new_at_path", level = "debug", skip(path))]
    pub fn new_at_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (conn, path) = crate::cache::open_with_migrations_infallible(
            &path,
            crate::cache::table::APP_MIGRATIONS
                .iter()
                .flat_map(|per_table| per_table.iter())
                .copied(),
        );
        Self {
            path: path.into(),
            conn,
        }
    }
}

impl std::fmt::Debug for AppCacheHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppCacheHandle")
            .field("db", &self.path)
            .finish()
    }
}

/// Lifecycle
impl CacheHandle {
    /// Infallible constructor that opens the project-local cache from `dir`,
    /// with an infallible fallback to an in-memory database.
    pub fn new_in_directory(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        if !dir.exists()
            && let Err(err) = std::fs::create_dir_all(dir)
        {
            tracing::warn!(
                ?err,
                "Failed to create project cache directory at {dir}, falling back to in-memory",
                dir = dir.display()
            );
        }
        Self::new_at_path(dir.join("but_cache.sqlite"))
    }

    /// Create a new instance at `path`.
    #[instrument(name = "CacheHandle::new_at_path", level = "debug", skip(path))]
    pub fn new_at_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (conn, path) = crate::cache::open_with_migrations_infallible(
            &path,
            crate::cache::table::PROJECT_MIGRATIONS
                .iter()
                .flat_map(|per_table| per_table.iter())
                .copied(),
        );
        Self {
            path: path.into(),
            conn,
        }
    }
}

impl std::fmt::Debug for CacheHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheHandle")
            .field("db", &self.path)
            .finish()
    }
}
