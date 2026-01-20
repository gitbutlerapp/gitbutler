mod table;
mod handle {
    use std::path::PathBuf;

    #[test]
    fn new_in_directory_none() {
        let handle = but_db::AppCacheHandle::new_in_directory(None::<PathBuf>);
        assert_eq!(
            format!("{handle:?}"),
            r#"AppCacheHandle { db: ":memory:" }"#,
            "falls back to in-memory if no directory is available"
        );
    }
}

/// Return a valid cache handle with all migrations applied, ready for use, and *in-memory* only.
fn in_memory_cache() -> but_db::AppCacheHandle {
    but_db::AppCacheHandle::new_at_url(":memory:")
}
