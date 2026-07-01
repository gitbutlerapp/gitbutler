use but_db::FileWriteLock;

use crate::table::in_memory_db;

#[test]
fn insert_and_read() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let lock1 = file_write_lock("path/to/file1.txt", "owner1");
    let lock2 = file_write_lock("path/to/file2.txt", "owner2");

    db.file_write_locks_mut().insert(lock1.clone())?;
    db.file_write_locks_mut().insert(lock2.clone())?;

    let locks = db.file_write_locks().list()?;
    assert_eq!(locks.len(), 2);
    assert!(locks.contains(&lock1));
    assert!(locks.contains(&lock2));

    Ok(())
}

#[test]
fn insert_replaces_existing() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let path_is_identity = "path/to/file.txt";
    let lock1 = file_write_lock(path_is_identity, "owner1");
    let lock2 = file_write_lock(path_is_identity, "owner2");

    db.file_write_locks_mut().insert(lock1)?;
    db.file_write_locks_mut().insert(lock2.clone())?;

    let locks = db.file_write_locks().list()?;
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0], lock2);

    Ok(())
}

#[test]
fn delete_lock() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let lock1 = file_write_lock("path/to/file1.txt", "owner1");
    let lock2 = file_write_lock("path/to/file2.txt", "owner2");

    db.file_write_locks_mut().insert(lock1)?;
    db.file_write_locks_mut().insert(lock2.clone())?;

    db.file_write_locks_mut().delete("path/to/file1.txt")?;

    let locks = db.file_write_locks().list()?;
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0], lock2);

    Ok(())
}

#[test]
fn empty_list() -> anyhow::Result<()> {
    let db = in_memory_db();

    let locks = db.file_write_locks().list()?;
    assert!(locks.is_empty());

    Ok(())
}

#[test]
fn with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let lock1 = file_write_lock("path/to/file1.txt", "owner1");
    let lock2 = file_write_lock("path/to/file2.txt", "owner2");

    let mut trans = db.transaction()?;
    trans.file_write_locks_mut().insert(lock1.clone())?;
    trans.file_write_locks_mut().insert(lock2.clone())?;

    let locks = trans.file_write_locks().list()?;
    assert_eq!(locks.len(), 2);

    trans.commit()?;

    let locks = db.file_write_locks().list()?;
    assert_eq!(locks.len(), 2);
    assert_eq!(locks, [lock1, lock2]);

    Ok(())
}

#[test]
fn transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let lock1 = file_write_lock("path/to/file.txt", "owner1");

    db.file_write_locks_mut().insert(lock1.clone())?;

    let mut trans = db.transaction()?;
    trans.file_write_locks_mut().delete("path/to/file.txt")?;
    trans.rollback()?;

    let locks = db.file_write_locks().list()?;
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0], lock1);

    Ok(())
}

fn file_write_lock(path: &str, owner: &str) -> FileWriteLock {
    FileWriteLock {
        path: path.to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        owner: owner.to_string(),
    }
}
