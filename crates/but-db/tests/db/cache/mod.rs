pub(crate) mod table;

use but_db::AppCacheHandle;

/// Return a valid cache handle with all migrations applied, ready for use, and *in-memory* only.
pub(crate) fn in_memory_cache() -> AppCacheHandle {
    AppCacheHandle::new_at_url(":memory:")
}
