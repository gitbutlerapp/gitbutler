use but_db::{DbHandle, HunkAssignment};
use rusqlite::ErrorCode;

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

fn two_handles_same_db() -> anyhow::Result<(DbHandle, DbHandle, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    let db1 = DbHandle::new_in_directory(tmp.path())?;
    let db2 = DbHandle::new_in_directory(tmp.path())?;
    Ok((db1, db2, tmp))
}
