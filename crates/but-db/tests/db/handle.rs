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
