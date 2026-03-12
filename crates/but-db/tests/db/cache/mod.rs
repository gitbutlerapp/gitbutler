mod table;
mod handle {
    mod app_cache_handle {
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

    mod cache_handle {
        const MEM_DB_DEBUG: &str = r#"CacheHandle { db: ":memory:" }"#;

        #[test]
        fn new_in_directory_some() -> anyhow::Result<()> {
            let tmp = tempfile::TempDir::new()?;

            let handle = but_db::CacheHandle::new_in_directory(tmp.path());
            assert_ne!(
                format!("{handle:?}"),
                MEM_DB_DEBUG,
                "it won't use a memory database if the directory exists"
            );
            Ok(())
        }

        #[test]
        fn new_in_nonexisting_dir() -> anyhow::Result<()> {
            let tmp = tempfile::TempDir::new()?;
            let handle = but_db::CacheHandle::new_in_directory(tmp.path().join("does-not-exist"));
            assert_ne!(
                format!("{handle:?}"),
                MEM_DB_DEBUG,
                "directories are created on demand for project-local caches"
            );
            Ok(())
        }

        #[test]
        fn new_falls_back_to_memory_if_unwritable() -> anyhow::Result<()> {
            // Use a directory path where a file-backed database cannot be created reliably
            let tmp = tempfile::TempDir::new()?;
            let handle = but_db::CacheHandle::new_at_path(tmp.path());
            assert_eq!(
                format!("{handle:?}"),
                MEM_DB_DEBUG,
                "permanent failures to open fall back to an in-memory project cache"
            );
            Ok(())
        }
    }
}

/// Return a valid cache handle with all migrations applied, ready for use, and *in-memory* only.
fn in_memory_cache() -> but_db::AppCacheHandle {
    but_db::AppCacheHandle::new_at_path(":memory:")
}

/// Return a valid project-local cache handle with all migrations applied, ready for use, and *in-memory* only.
fn in_memory_project_cache() -> but_db::CacheHandle {
    but_db::CacheHandle::new_at_path(":memory:")
}
