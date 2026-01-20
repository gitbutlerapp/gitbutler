use crate::AppCacheHandle;
use std::path::Path;

/// Lifecycle
impl AppCacheHandle {
    /// Infallible constructor that opens from `dir` or from memory if that is `None`,
    /// with an infallible constructor that falls back to an in-memory database.
    pub fn new_in_directory(dir: Option<impl AsRef<Path>>) -> Self {
        let url = dir.map_or(":memory:".into(), |d| d.as_ref().join("app-cache.sqlite"));
        Self::new_at_url(
            url.to_str()
                .expect("BUG: application wide cache directories should always be valid UTF-8"),
        )
    }

    /// Create a new instance at `url`.
    pub fn new_at_url(url: impl Into<String>) -> Self {
        let url = url.into();
        let (conn, url) = crate::cache::open_with_migrations_infallible(
            &url,
            crate::cache::table::APP_MIGRATIONS
                .iter()
                .flat_map(|per_table| per_table.iter())
                .copied(),
        );
        Self {
            url: url.into(),
            conn,
        }
    }
}

impl std::fmt::Debug for AppCacheHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppCacheHandle")
            .field("db", &self.url)
            .finish()
    }
}
