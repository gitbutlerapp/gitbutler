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
    let order: [gix::refs::FullName; 2] = ["refs/heads/A".try_into()?, "refs/heads/B".try_into()?];
    {
        let mut db = DbHandle::new_in_directory(tmp.path())?;
        db.meta_mut()?.set_branch_stack_order(&order)?;
    }

    let db = DbHandle::open_existing_read_only_in_directory(tmp.path())?
        .expect("database was created before read-only open");

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/B".try_into()?),
        Some(order.as_slice()),
        "read-only handles should see existing branch order"
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
