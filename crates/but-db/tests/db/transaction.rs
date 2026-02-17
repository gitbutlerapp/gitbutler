use but_db::{DbHandle, HunkAssignment};
use rusqlite::{ErrorCode, ffi};

#[test]
fn deferred() -> anyhow::Result<()> {
    let (mut db1, mut db2, _tmp) = two_handles_same_db()?;
    // Deferred transactions can be interleaved.
    let mut t1 = db1.transaction()?;
    let mut t2 = db2.transaction()?;

    // Savepoints can be interleaved
    let ha1 = t1.hunk_assignments_mut()?;

    t2.set_nonblocking()?;
    let ha2 = t2.hunk_assignments_mut()?;

    let assignments = vec![HunkAssignment {
        id: None,
        hunk_header: None,
        path: "ha1".into(),
        path_bytes: vec![],
        stack_id: None,
    }];
    ha1.set_all(assignments.clone())
        .expect("first transaction gets the lock");

    let err = ha2.set_all(assignments.clone()).unwrap_err();
    assert_eq!(
        err.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy),
        "the second transaction would block because the lock is taken"
    );
    assert_eq!(
        t2.hunk_assignments().list_all().expect("readers see the original data"),
        [],
        "However, thanks to WAL we can still read."
    );

    t1.commit()?;

    assert_eq!(
        t2.hunk_assignments().list_all()?,
        vec![],
        "the data is written through t1, we see the state when creating the transaction"
    );

    assert_eq!(
        db1.hunk_assignments().list_all()?,
        assignments,
        "the data is visible through the connection, it sees the latest state"
    );
    assert_eq!(
        db1.transaction()?.hunk_assignments().list_all()?,
        assignments,
        "the data is visible through a new transaction as well"
    );

    Ok(())
}

#[test]
fn immediate() -> anyhow::Result<()> {
    let (mut db1, mut db2, _tmp) = two_handles_same_db()?;
    // Get the write lock.
    let t1 = db1.immediate_transaction()?;
    assert!(db2.immediate_transaction_nonblocking()?.is_none());
    t1.commit()?;

    assert!(
        db2.immediate_transaction_nonblocking()?.is_some(),
        "now we get a transaction as the other lock is dropped"
    );
    Ok(())
}

#[test]
fn deferred_savepoint_write_after_concurrent_commit_returns_busy_snapshot() -> anyhow::Result<()> {
    let (mut db1, mut db2, _tmp) = two_handles_same_db()?;

    // Start two deferred transactions and establish a read snapshot in t2.
    let mut t1 = db1.transaction()?;
    let mut t2 = db2.transaction()?;
    assert_eq!(t2.hunk_assignments().list_all()?, vec![]);

    // Write through a savepoint in t1 and commit the outer transaction.
    let assignments = vec![HunkAssignment {
        id: None,
        hunk_header: None,
        path: "from_t1".into(),
        path_bytes: vec![],
        stack_id: None,
    }];
    t1.hunk_assignments_mut()?.set_all(assignments.clone())?;
    t1.commit()?;

    // t2 tries to write through a savepoint using its stale read snapshot.
    let err = t2
        .hunk_assignments_mut()?
        .set_all(assignments.clone())
        .expect_err("stale read snapshot cannot be promoted to writer");
    assert_eq!(err.sqlite_error_code(), Some(ErrorCode::DatabaseBusy));
    assert_eq!(
        err.sqlite_error().map(|err| err.extended_code),
        Some(ffi::SQLITE_BUSY_SNAPSHOT),
        "expected BUSY_SNAPSHOT extended code for stale snapshot write-upgrade"
    );

    assert_eq!(
        t2.hunk_assignments().list_all()?,
        vec![],
        "A savepoint rollback does not refresh the outer transaction snapshot."
    );

    // Recovery requires a fresh transaction.
    drop(t2);
    let t2 = db2.transaction()?;
    assert_eq!(
        t2.hunk_assignments().list_all()?,
        assignments,
        "now the prior write is observable"
    );

    Ok(())
}

fn two_handles_same_db() -> anyhow::Result<(DbHandle, DbHandle, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    let db1 = DbHandle::new_in_directory(tmp.path())?;
    let db2 = DbHandle::new_in_directory(tmp.path())?;
    Ok((db1, db2, tmp))
}
