mod table;
mod handle {
    use std::path::PathBuf;

    const MEM_DB_DEBUG: &str = r#"AppCacheHandle { db: ":memory:" }"#;

    #[test]
    fn new_in_directory_none() {
        let handle = but_db::AppCacheHandle::new_in_directory(None::<PathBuf>);
        assert_eq!(
            format!("{handle:?}"),
            MEM_DB_DEBUG,
            "falls back to in-memory if no directory is available"
        );
    }

    #[test]
    fn new_in_directory_some() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;

        let handle = but_db::AppCacheHandle::new_in_directory(tmp.path().into());
        assert_ne!(
            format!("{handle:?}"),
            MEM_DB_DEBUG,
            "it won't use a memory database if the directory exist"
        );
        Ok(())
    }
}

/// Return a valid cache handle with all migrations applied, ready for use, and *in-memory* only.
fn in_memory_cache() -> but_db::AppCacheHandle {
    but_db::AppCacheHandle::new_at_path(":memory:")
}
