use but_db::DbHandle;

#[test]
fn basic_usage() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let db = DbHandle::new_in_directory(tmp.path())?;
    assert!(db.hunk_assignments().list_all()?.is_empty());

    // Two handles at the same time.
    let other_db = DbHandle::new_in_directory(tmp.path())?;
    assert!(other_db.hunk_assignments().list_all()?.is_empty());

    assert!(
        tmp.path().join("but.sqlite").exists(),
        "The database file is well-known and is auto-created"
    );
    Ok(())
}

#[test]
fn in_nonexisting_dir() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let db = DbHandle::new_in_directory(tmp.path().join("does-not-exist"))?;
    assert!(
        db.hunk_assignments().list_all()?.is_empty(),
        "directories are created on demand, otherwise initialization fails, fair enough"
    );
    Ok(())
}

#[test]
fn read_only_does_not_create_missing_database() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;

    let db = DbHandle::open_existing_read_only_in_directory(tmp.path())?;

    assert!(
        db.is_none(),
        "missing databases should not be created by read-only opens"
    );
    assert!(
        !tmp.path().join("but.sqlite").exists(),
        "read-only opens should leave the filesystem untouched"
    );
    Ok(())
}

#[test]
fn read_only_observes_existing_database() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    {
        let mut db = DbHandle::new_in_directory(tmp.path())?;
        db.branch_order_mut()?
            .set_order(&["refs/heads/A".to_owned(), "refs/heads/B".to_owned()])?;
    }

    let db = DbHandle::open_existing_read_only_in_directory(tmp.path())?
        .expect("database was created before read-only open");

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/B")?,
        Some(vec!["refs/heads/A".to_owned(), "refs/heads/B".to_owned()]),
        "read-only handles should see existing branch order"
    );
    Ok(())
}

#[test]
fn read_only_clone_discards_writes() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    {
        let mut source = DbHandle::new_in_directory(tmp.path())?;
        source
            .branch_order_mut()?
            .set_order(&["refs/heads/A".to_owned()])?;
    }

    {
        let mut clone = DbHandle::open_existing_read_only_in_directory(tmp.path())?
            .expect("database was created before read-only open");
        clone
            .branch_order_mut()?
            .set_order(&["refs/heads/A".to_owned(), "refs/heads/B".to_owned()])?;
        assert_eq!(
            clone.branch_order().order_for_reference("refs/heads/B")?,
            Some(vec!["refs/heads/A".to_owned(), "refs/heads/B".to_owned()]),
            "the private clone should accept process-local writes"
        );
    }
    assert!(
        !tmp.path().join("but.sqlite-wal").exists() && !tmp.path().join("but.sqlite-shm").exists(),
        "cloning a checkpointed database must not create source sidecars"
    );

    let source = DbHandle::new_in_directory(tmp.path())?;
    assert_eq!(
        source.branch_order().order_for_reference("refs/heads/B")?,
        None,
        "dropping the clone should discard its writes"
    );
    assert_eq!(
        source.branch_order().order_for_reference("refs/heads/A")?,
        Some(vec!["refs/heads/A".to_owned()]),
        "the original source state should remain intact"
    );
    Ok(())
}

#[test]
fn read_only_observes_database_at_uri_sensitive_path() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let db_dir = tmp.path().join("project data ? read only");
    {
        let mut db = DbHandle::new_in_directory(&db_dir)?;
        db.branch_order_mut()?
            .set_order(&["refs/heads/A".to_owned()])?;
    }

    let db = DbHandle::open_existing_read_only_in_directory(&db_dir)?
        .expect("database was created before read-only open");
    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/A")?,
        Some(vec!["refs/heads/A".to_owned()])
    );
    Ok(())
}

#[test]
fn read_only_observes_current_wal_while_writer_remains_open() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let mut writer = DbHandle::new_in_directory(tmp.path())?;
    writer
        .branch_order_mut()?
        .set_order(&["refs/heads/A".to_owned()])?;
    assert!(tmp.path().join("but.sqlite-wal").exists());

    let first = DbHandle::open_existing_read_only_in_directory(tmp.path())?
        .expect("writer created the database");
    assert_eq!(
        first.branch_order().order_for_reference("refs/heads/A")?,
        Some(vec!["refs/heads/A".to_owned()])
    );

    writer
        .branch_order_mut()?
        .set_order(&["refs/heads/A".to_owned(), "refs/heads/B".to_owned()])?;
    let second = DbHandle::open_existing_read_only_in_directory(tmp.path())?
        .expect("writer kept the database open");
    assert_eq!(
        second.branch_order().order_for_reference("refs/heads/B")?,
        Some(vec!["refs/heads/A".to_owned(), "refs/heads/B".to_owned()])
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn read_only_clone_works_when_directory_mode_overstates_writability() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let tmp = tempfile::tempdir()?;
    {
        let mut writer = DbHandle::new_in_directory(tmp.path())?;
        writer
            .branch_order_mut()?
            .set_order(&["refs/heads/A".to_owned()])?;
    }
    let db_path = tmp.path().join("but.sqlite");
    let wal_path = tmp.path().join("but.sqlite-wal");
    let shm_path = tmp.path().join("but.sqlite-shm");
    assert!(
        !wal_path.exists() && !shm_path.exists(),
        "a cleanly closed WAL database should start without coordination sidecars"
    );
    let original_db = std::fs::read(&db_path)?;
    let original_dir_permissions = std::fs::metadata(tmp.path())?.permissions();

    // The owner can traverse and read the directory but cannot create files. Group and other
    // write bits keep Permissions::readonly() false, reproducing a sandbox policy that is stricter
    // than the visible Unix mode bits.
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o577))?;
    let open_result = (|| -> anyhow::Result<()> {
        let clone = DbHandle::open_existing_read_only_in_directory(tmp.path())?
            .expect("database existed before directory writes were denied");
        assert_eq!(
            clone.branch_order().order_for_reference("refs/heads/A")?,
            Some(vec!["refs/heads/A".to_owned()]),
            "the private snapshot must preserve the last checkpointed state"
        );
        Ok(())
    })();
    std::fs::set_permissions(tmp.path(), original_dir_permissions)?;

    open_result?;
    assert_eq!(
        std::fs::read(&db_path)?,
        original_db,
        "the private snapshot must not modify the source database"
    );
    assert!(
        !wal_path.exists() && !shm_path.exists(),
        "the private snapshot must not create source coordination sidecars"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn read_only_clone_opens_physically_read_only_database_without_sidecars() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let tmp = tempfile::tempdir()?;
    {
        let mut writer = DbHandle::new_in_directory(tmp.path())?;
        writer
            .branch_order_mut()?
            .set_order(&["refs/heads/A".to_owned()])?;
    }
    let db_path = tmp.path().join("but.sqlite");
    let wal_path = tmp.path().join("but.sqlite-wal");
    let shm_path = tmp.path().join("but.sqlite-shm");
    assert!(!wal_path.exists() && !shm_path.exists());
    let original_db = std::fs::read(&db_path)?;
    let original_db_permissions = std::fs::metadata(&db_path)?.permissions();
    let original_dir_permissions = std::fs::metadata(tmp.path())?.permissions();
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o444))?;
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555))?;

    let open_result = (|| -> anyhow::Result<()> {
        let clone = DbHandle::open_existing_read_only_in_directory(tmp.path())?
            .expect("database existed before its directory became read-only");
        assert_eq!(
            clone.branch_order().order_for_reference("refs/heads/A")?,
            Some(vec!["refs/heads/A".to_owned()])
        );
        Ok(())
    })();

    std::fs::set_permissions(tmp.path(), original_dir_permissions)?;
    std::fs::set_permissions(&db_path, original_db_permissions)?;
    open_result?;
    assert_eq!(std::fs::read(&db_path)?, original_db);
    assert!(
        !wal_path.exists() && !shm_path.exists(),
        "opening a physically read-only database must not require sidecars"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn read_only_clone_observes_wal_in_physically_read_only_database() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let source = tempfile::tempdir()?;
    let mut writer = DbHandle::new_in_directory(source.path())?;
    writer
        .branch_order_mut()?
        .set_order(&["refs/heads/A".to_owned()])?;
    let tmp = tempfile::tempdir()?;
    for name in ["but.sqlite", "but.sqlite-wal", "but.sqlite-shm"] {
        std::fs::copy(source.path().join(name), tmp.path().join(name))?;
    }
    let paths = [
        tmp.path().join("but.sqlite"),
        tmp.path().join("but.sqlite-wal"),
        tmp.path().join("but.sqlite-shm"),
    ];
    let original_files = paths
        .iter()
        .map(|path| Ok((std::fs::read(path)?, std::fs::metadata(path)?.permissions())))
        .collect::<std::io::Result<Vec<_>>>()?;
    let original_dir_permissions = std::fs::metadata(tmp.path())?.permissions();
    for path in &paths {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o444))?;
    }
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555))?;

    let open_result = (|| -> anyhow::Result<()> {
        let clone = DbHandle::open_existing_read_only_in_directory(tmp.path())?
            .expect("database existed before its directory became read-only");
        assert_eq!(
            clone.branch_order().order_for_reference("refs/heads/A")?,
            Some(vec!["refs/heads/A".to_owned()]),
            "the in-memory clone must include committed source WAL state"
        );
        Ok(())
    })();

    std::fs::set_permissions(tmp.path(), original_dir_permissions)?;
    for (path, (_, permissions)) in paths.iter().zip(&original_files) {
        std::fs::set_permissions(path, permissions.clone())?;
    }
    open_result?;
    for (path, (bytes, _)) in paths.iter().zip(&original_files) {
        assert_eq!(
            std::fs::read(path)?,
            *bytes,
            "read-only cloning must not modify source state in {}",
            path.display()
        );
    }
    drop(writer);
    Ok(())
}

#[cfg(unix)]
#[test]
fn read_only_reports_unreadable_wal_without_modifying_it() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let tmp = tempfile::tempdir()?;
    let mut writer = DbHandle::new_in_directory(tmp.path())?;
    writer
        .branch_order_mut()?
        .set_order(&["refs/heads/A".to_owned()])?;
    let wal_path = tmp.path().join("but.sqlite-wal");
    let wal_bytes = std::fs::read(&wal_path)?;
    let original_permissions = std::fs::metadata(&wal_path)?.permissions();
    std::fs::set_permissions(&wal_path, std::fs::Permissions::from_mode(0o0))?;

    let err = DbHandle::open_existing_read_only_in_directory(tmp.path())
        .expect_err("SQLite must report an unreadable WAL instead of returning a partial clone");
    std::fs::set_permissions(&wal_path, original_permissions)?;

    assert!(
        err.chain().any(|cause| cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)),
        "PermissionDenied should remain downcastable in the error chain: {err:#}"
    );
    assert_eq!(
        std::fs::read(&wal_path)?,
        wal_bytes,
        "a failed in-memory backup must not modify the source WAL"
    );
    Ok(())
}

#[test]
fn in_parallel_with_threads() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let num_threads = 2;
    let barrier = std::sync::Barrier::new(num_threads);
    std::thread::scope(|scope| {
        for _n in 0..num_threads {
            scope.spawn(|| -> anyhow::Result<_> {
                barrier.wait();
                for _round in 0..10 {
                    let handle = DbHandle::new_in_directory(tmp.path())?;
                    assert!(handle.hunk_assignments().list_all()?.is_empty());
                }
                Ok(())
            });
        }
    });
    Ok(())
}
