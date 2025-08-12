use but_db::DbHandle;

#[test]
fn init_and_basic_usage() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(tmp.path())?;
    assert!(db.hunk_assignments().list_all()?.is_empty());

    // Two handles at the same time.
    let mut other_db = DbHandle::new_in_directory(tmp.path())?;
    assert!(other_db.hunk_assignments().list_all()?.is_empty());

    assert!(
        tmp.path().join("but.sqlite").exists(),
        "The database file is well-known and is auto-created"
    );
    Ok(())
}

#[test]
fn init_in_nonexisting_dir() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(tmp.path().join("does-not-exist"))?;
    assert!(
        db.hunk_assignments().list_all()?.is_empty(),
        "directories are created on demand, otherwise initialization fails, fair enough"
    );
    Ok(())
}

#[test]
fn init_in_parallel() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let num_threads = 2;
    let barrier = std::sync::Barrier::new(num_threads);
    std::thread::scope(|scope| {
        for _n in 0..num_threads {
            scope.spawn(|| -> anyhow::Result<_> {
                barrier.wait();
                for _round in 0..10 {
                    let mut handle = DbHandle::new_in_directory(tmp.path())?;
                    assert!(handle.hunk_assignments().list_all()?.is_empty());
                }
                Ok(())
            });
        }
    });
    Ok(())
}
