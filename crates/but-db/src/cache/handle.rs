use std::path::{Path, PathBuf};

use tracing::instrument;

use crate::{AppCacheHandle, CacheHandle};

impl AppCacheHandle {
    /// Falls back to an in-memory database if `dir` is `None` or can't be opened.
    pub fn new_in_directory(dir: Option<impl AsRef<Path>>) -> Self {
        let db_path = dir.map_or(":memory:".into(), |d| d.as_ref().join("app-cache.sqlite"));
        if let Some(cache_dir) = db_path.parent() {
            create_dir_all_log_failure(cache_dir);
        }
        Self::new_at_path(db_path)
    }

    #[instrument(name = "AppCacheHandle::new_at_path", level = "trace", skip(path))]
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

impl CacheHandle {
    /// Falls back to an in-memory database if `dir` can't be opened.
    pub fn new_in_directory(dir: impl AsRef<Path>) -> Self {
        create_dir_all_log_failure(&dir);
        Self::new_at_path(dir.as_ref().join("but_cache.sqlite"))
    }

    #[instrument(name = "CacheHandle::new_at_path", level = "trace", skip(path))]
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

fn create_dir_all_log_failure(dir: impl AsRef<Path>) {
    if let Err(err) = std::fs::create_dir_all(&dir) {
        tracing::warn!(
            ?err,
            "Failed to create project cache directory at {dir}, skipping ...",
            dir = dir.as_ref().display()
        );
    }
}
